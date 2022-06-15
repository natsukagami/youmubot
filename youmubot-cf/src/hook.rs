use chrono::{TimeZone, Utc};
use codeforces::{Contest, Problem};
use dashmap::DashMap as HashMap;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serenity::{
    builder::CreateEmbed, framework::standard::CommandError, model::channel::Message,
    utils::MessageBuilder,
};
use std::collections::HashMap as StdHashMap;
use std::time::Instant;
use youmubot_prelude::*;

type Client = <crate::CFClient as TypeMapKey>::Value;

lazy_static! {
    static ref CONTEST_LINK: Regex = Regex::new(
        r"https?://codeforces\.com/(contest|gym)s?/(?P<contest>\d+)(?:/problem/(?P<problem>\w+))?"
    )
    .unwrap();
    static ref PROBLEMSET_LINK: Regex = Regex::new(
        r"https?://codeforces\.com/problemset/problem/(?P<contest>\d+)/(?P<problem>\w+)"
    )
    .unwrap();
}

enum ContestOrProblem {
    Contest(Contest, Option<Vec<Problem>>),
    Problem(Problem),
}

/// Caches the contest list.
pub struct ContestCache {
    contests: HashMap<u64, (Contest, Option<Vec<Problem>>)>,
    all_list: RwLock<(StdHashMap<u64, Contest>, Instant)>,
    http: Client,
}

impl TypeMapKey for ContestCache {
    type Value = ContestCache;
}

impl ContestCache {
    /// Creates a new, empty cache.
    pub(crate) async fn new(http: Client) -> Result<Self> {
        let contests_list = Self::fetch_contest_list(http.clone()).await?;
        Ok(Self {
            contests: HashMap::new(),
            all_list: RwLock::new((contests_list, Instant::now())),
            http,
        })
    }

    async fn fetch_contest_list(http: Client) -> Result<StdHashMap<u64, Contest>> {
        log::info!("Fetching contest list, this might take a few seconds to complete...");
        let gyms = Contest::list(&*http, true).await?;
        let contests = Contest::list(&*http, false).await?;
        let r: StdHashMap<u64, Contest> = gyms
            .into_iter()
            .chain(contests.into_iter())
            .map(|v| (v.id, v))
            .collect();
        log::info!("Contest list fetched, {} contests indexed", r.len());
        Ok(r)
    }

    /// Gets a contest from the cache, fetching from upstream if possible.
    pub async fn get(&self, contest_id: u64) -> Result<(Contest, Option<Vec<Problem>>)> {
        if let Some(v) = self.contests.get(&contest_id) {
            if v.1.is_some() {
                return Ok(v.clone());
            }
        }
        self.get_and_store_contest(contest_id).await
    }

    async fn get_and_store_contest(
        &self,
        contest_id: u64,
    ) -> Result<(Contest, Option<Vec<Problem>>)> {
        let (c, p) = match Contest::standings(&*self.http, contest_id, |f| f.limit(1, 1)).await {
            Ok((c, p, _)) => (c, Some(p)),
            Err(codeforces::Error::Codeforces(s)) if s.ends_with("has not started") => {
                let c = self.get_from_list(contest_id).await?;
                (c, None)
            }
            Err(v) => return Err(Error::from(v)),
        };
        self.contests.insert(contest_id, (c, p));
        Ok(self.contests.get(&contest_id).unwrap().clone())
    }

    async fn get_from_list(&self, contest_id: u64) -> Result<Contest> {
        let last_updated = self.all_list.read().await.1;
        if Instant::now() - last_updated > std::time::Duration::from_secs(60 * 60) {
            // We update at most once an hour.
            let mut v = self.all_list.write().await;
            *v = (
                Self::fetch_contest_list(self.http.clone()).await?,
                Instant::now(),
            );
        }
        self.all_list
            .read()
            .await
            .0
            .get(&contest_id)
            .cloned()
            .ok_or_else(|| Error::msg("Contest not found"))
    }
}

/// Prints info whenever a problem or contest (or more) is sent on a channel.
pub struct InfoHook;

#[async_trait]
impl Hook for InfoHook {
    async fn call(&mut self, ctx: &Context, m: &Message) -> Result<()> {
        if m.author.bot {
            return Ok(());
        }
        let data = ctx.data.read().await;
        let contest_cache = data.get::<ContestCache>().unwrap();
        let matches = parse(&m.content[..], contest_cache)
            .collect::<Vec<_>>()
            .await;
        if !matches.is_empty() {
            m.channel_id
                .send_message(&ctx, |c| {
                    c.content("Here are the info of the given Codeforces links!")
                        .embed(|e| print_info_message(&matches[..], e))
                })
                .await?;
        }
        Ok(())
    }
}

fn parse<'a>(
    content: &'a str,
    contest_cache: &'a ContestCache,
) -> impl stream::Stream<Item = (ContestOrProblem, &'a str)> + 'a {
    let matches = CONTEST_LINK
        .captures_iter(content)
        .chain(PROBLEMSET_LINK.captures_iter(content))
        .map(|v| parse_capture(contest_cache, v))
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| future::ready(v.ok()));
    matches
}

fn print_info_message<'a>(
    info: &[(ContestOrProblem, &str)],
    e: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let (problems, contests): (Vec<_>, Vec<_>) = info.iter().partition(|(v, _)| match v {
        ContestOrProblem::Problem(_) => true,
        ContestOrProblem::Contest(_, _) => false,
    });
    let mut problems = problems
        .into_iter()
        .map(|(v, l)| match v {
            ContestOrProblem::Problem(p) => (p, l),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    let contests = contests
        .into_iter()
        .map(|(v, l)| match v {
            ContestOrProblem::Contest(c, p) => (c, p, l),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    problems.sort_by(|(a, _), (b, _)| a.rating.unwrap_or(1500).cmp(&b.rating.unwrap_or(1500)));
    let mut m = MessageBuilder::new();
    if !problems.is_empty() {
        m.push_line("**Problems**").push_line("");
        for (problem, link) in problems {
            m.push(" - [")
                .push_bold_safe(format!(
                    "[{}{}] {}",
                    problem.contest_id.unwrap_or(0),
                    problem.index,
                    problem.name
                ))
                .push(format!("]({})", link));
            if let Some(p) = problem.points {
                m.push(format!(" | **{:.0}** points", p));
            }
            if let Some(p) = problem.rating {
                m.push(format!(" | rating **{:.0}**", p));
            }
            if !problem.tags.is_empty() {
                m.push(format!(" | tags: ||`{}`||", problem.tags.join(", ")));
            }
            m.push_line("");
        }
    }
    m.push_line("");

    if !contests.is_empty() {
        m.push_bold_line("Contests").push_line("");
        for (contest, problems, link) in contests {
            let duration = Duration::from_secs(contest.duration_seconds);
            m.push(" - [")
                .push_bold_safe(&contest.name)
                .push(format!("]({})", link))
                .push(format!(" | {}", contest.phase))
                .push(
                    problems
                        .as_ref()
                        .map(|v| format!(" | **{}** problems", v.len()))
                        .unwrap_or_else(|| "".to_owned()),
                )
                .push(
                    contest
                        .start_time_seconds
                        .as_ref()
                        .map(|v| {
                            let ts = Utc.timestamp(*v as i64, 0);
                            format!(
                                " | from {} ({})",
                                ts.format("<t:%s:F>"),
                                ts.format("<t:%s:R>")
                            )
                        })
                        .unwrap_or_else(|| "".to_owned()),
                )
                .push(format!(" | duration **{}**", duration));
            if let Some(p) = &contest.prepared_by {
                m.push(format!(
                    " | prepared by [{}](https://codeforces.com/profile/{})",
                    p, p
                ));
            }
            m.push_line("");
        }
    }
    e.description(m.build())
}

#[allow(clippy::needless_lifetimes)] // Doesn't really work
async fn parse_capture<'a>(
    contest_cache: &ContestCache,
    cap: Captures<'a>,
) -> Result<(ContestOrProblem, &'a str), CommandError> {
    let contest_id: u64 = cap
        .name("contest")
        .ok_or_else(|| CommandError::from("Contest not captured"))?
        .as_str()
        .parse()?;
    let (contest, problems) = contest_cache.get(contest_id).await?;
    match cap.name("problem") {
        Some(p) => {
            for problem in problems.ok_or_else(|| CommandError::from("Contest hasn't started"))? {
                if problem.index == p.as_str() {
                    return Ok((
                        ContestOrProblem::Problem(problem),
                        cap.get(0).unwrap().as_str(),
                    ));
                }
            }
            Err("No such problem in contest".into())
        }
        None => Ok((
            ContestOrProblem::Contest(contest, problems),
            cap.get(0).unwrap().as_str(),
        )),
    }
}

use codeforces::{Contest, Problem};
use lazy_static::lazy_static;
use rayon::{iter::Either, prelude::*};
use regex::{Captures, Regex};
use serenity::{
    builder::CreateEmbed, framework::standard::CommandError, model::channel::Message,
    utils::MessageBuilder,
};
use std::{collections::HashMap, sync::Arc};
use youmubot_prelude::*;

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
#[derive(Clone, Debug, Default)]
pub struct ContestCache(Arc<RwLock<HashMap<u64, (Contest, Option<Vec<Problem>>)>>>);

impl TypeMapKey for ContestCache {
    type Value = ContestCache;
}

impl ContestCache {
    fn get(
        &self,
        http: &<HTTPClient as TypeMapKey>::Value,
        contest_id: u64,
    ) -> Result<(Contest, Option<Vec<Problem>>), CommandError> {
        let rl = self.0.read();
        match rl.get(&contest_id) {
            Some(r @ (_, Some(_))) => Ok(r.clone()),
            Some((c, None)) => match Contest::standings(http, contest_id, |f| f.limit(1, 1)) {
                Ok((c, p, _)) => Ok({
                    drop(rl);
                    self.0
                        .write()
                        .entry(contest_id)
                        .or_insert((c, Some(p)))
                        .clone()
                }),
                Err(_) => Ok((c.clone(), None)),
            },
            None => {
                drop(rl);
                // Step 1: try to fetch it individually
                match Contest::standings(http, contest_id, |f| f.limit(1, 1)) {
                    Ok((c, p, _)) => Ok(self
                        .0
                        .write()
                        .entry(contest_id)
                        .or_insert((c, Some(p)))
                        .clone()),
                    Err(codeforces::Error::Codeforces(s)) if s.ends_with("has not started") => {
                        // Fetch the entire list
                        {
                            let mut m = self.0.write();
                            let contests = Contest::list(http, contest_id > 100_000)?;
                            contests.into_iter().for_each(|c| {
                                m.entry(c.id).or_insert((c, None));
                            });
                        }
                        self.0
                            .read()
                            .get(&contest_id)
                            .cloned()
                            .ok_or("No contest found".into())
                    }
                    Err(e) => Err(e.into()),
                }
                // Step 2: try to fetch the entire list.
            }
        }
    }
}

/// Prints info whenever a problem or contest (or more) is sent on a channel.
pub fn codeforces_info_hook(ctx: &mut Context, m: &Message) {
    if m.author.bot {
        return;
    }
    let http = ctx.data.get_cloned::<HTTPClient>();
    let contest_cache = ctx.data.get_cloned::<ContestCache>();
    let matches = CONTEST_LINK
        .captures_iter(&m.content)
        .chain(PROBLEMSET_LINK.captures_iter(&m.content))
        // .collect::<Vec<_>>()
        // .into_par_iter()
        .filter_map(
            |v| match parse_capture(http.clone(), contest_cache.clone(), v) {
                Ok(v) => Some(v),
                Err(e) => {
                    dbg!(e);
                    None
                }
            },
        )
        .collect::<Vec<_>>();
    if !matches.is_empty() {
        m.channel_id
            .send_message(&ctx, |c| {
                c.content("Here are the info of the given Codeforces links!")
                    .embed(|e| print_info_message(&matches[..], e))
            })
            .ok();
    }
}

fn print_info_message<'a>(
    info: &[(ContestOrProblem, &str)],
    e: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let (mut problems, contests): (Vec<_>, Vec<_>) =
        info.par_iter().partition_map(|(v, l)| match v {
            ContestOrProblem::Problem(p) => Either::Left((p, l)),
            ContestOrProblem::Contest(c, p) => Either::Right((c, p, l)),
        });
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
            let duration: Duration = format!("{}s", contest.duration_seconds).parse().unwrap();
            m.push(" - [")
                .push_bold_safe(&contest.name)
                .push(format!("]({})", link))
                .push(format!(
                    " | {} | duration **{}**",
                    problems
                        .as_ref()
                        .map(|v| format!("{} | **{}** problems", contest.phase, v.len()))
                        .unwrap_or(format!("{}", contest.phase)),
                    duration
                ));
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

fn parse_capture<'a>(
    http: <HTTPClient as TypeMapKey>::Value,
    contest_cache: ContestCache,
    cap: Captures<'a>,
) -> Result<(ContestOrProblem, &'a str), CommandError> {
    let contest_id: u64 = cap
        .name("contest")
        .ok_or(CommandError::from("Contest not captured"))?
        .as_str()
        .parse()?;
    let (contest, problems) = contest_cache.get(&http, contest_id)?;
    match cap.name("problem") {
        Some(p) => {
            for problem in problems.ok_or(CommandError::from("Contest hasn't started"))? {
                if &problem.index == p.as_str() {
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

use codeforces::{Contest, Problem};
use lazy_static::lazy_static;
use rayon::{iter::Either, prelude::*};
use regex::{Captures, Regex};
use serenity::{
    builder::CreateEmbed,
    framework::standard::{CommandError, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use youmubot_prelude::*;

lazy_static! {
    static ref CONTEST_LINK: Regex = Regex::new(
        r"https?://codeforces\.com/(contest|gym)/(?P<contest>\d+)(?:/problem/(?P<problem>\w+))?"
    )
    .unwrap();
    static ref PROBLEMSET_LINK: Regex = Regex::new(
        r"https?://codeforces\.com/problemset/problem/(?P<contest>\d+)/(?P<problem>\w+)"
    )
    .unwrap();
}

enum ContestOrProblem {
    Contest(Contest, Vec<Problem>),
    Problem(Problem),
}

/// Prints info whenever a problem or contest (or more) is sent on a channel.
pub fn codeforces_info_hook(ctx: &mut Context, m: &Message) {
    if m.author.bot {
        return;
    }
    let http = ctx.data.get_cloned::<HTTPClient>();
    let matches = CONTEST_LINK
        .captures_iter(&m.content)
        .chain(PROBLEMSET_LINK.captures_iter(&m.content))
        // .collect::<Vec<_>>()
        // .into_par_iter()
        .filter_map(|v| match parse_capture(http.clone(), v) {
            Ok(v) => Some(v),
            Err(e) => {
                dbg!(e);
                None
            }
        })
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
                    " | **{}** problems | duration **{}**",
                    problems.len(),
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
    cap: Captures<'a>,
) -> Result<(ContestOrProblem, &'a str), CommandError> {
    let contest: u64 = cap
        .name("contest")
        .ok_or(CommandError::from("Contest not captured"))?
        .as_str()
        .parse()?;
    let (contest, problems, _) = Contest::standings(&http, contest, |f| f.limit(1, 1))?;
    match cap.name("problem") {
        Some(p) => {
            for problem in problems {
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

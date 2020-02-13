use crate::db::CfSavedUsers;
use codeforces::{Contest, ContestPhase, Problem, ProblemResult, ProblemResultType, RanklistRow};
use rayon::prelude::*;
use serenity::{
    framework::standard::{CommandError, CommandResult},
    model::{
        guild::Member,
        id::{ChannelId, GuildId, UserId},
    },
    utils::MessageBuilder,
};
use std::collections::HashMap;
use youmubot_prelude::*;

struct MemberResult {
    member: Member,
    handle: String,
    row: Option<RanklistRow>,
}

/// Watch and commentate a contest.
///
/// Does the thing on a channel, block until the contest ends.
pub fn watch_contest(
    ctx: &mut Context,
    guild: GuildId,
    channel: ChannelId,
    contest_id: u64,
) -> CommandResult {
    let db = CfSavedUsers::open(&*ctx.data.read()).borrow()?.clone();
    let http = ctx.http.clone();
    // Collect an initial member list.
    // This never changes during the scan.
    let mut member_results: HashMap<UserId, MemberResult> = db
        .into_par_iter()
        .filter_map(|(user_id, cfu)| {
            let member = guild.member(http.clone().as_ref(), user_id).ok();
            match member {
                Some(m) => Some((
                    user_id,
                    MemberResult {
                        member: m,
                        handle: cfu.handle,
                        row: None,
                    },
                )),
                None => None,
            }
        })
        .collect();

    let http = ctx.data.get_cloned::<HTTPClient>();
    let (mut contest, _, _) = Contest::standings(&http, contest_id, |f| f.limit(1, 1))?;

    channel.send_message(&ctx, |e| {
        e.content(format!(
            "Youmu is watching contest **{}**, with the following members:\n{}",
            contest.name,
            member_results
                .iter()
                .map(|(_, m)| format!("- {} as **{}**", m.member.distinct(), m.handle))
                .collect::<Vec<_>>()
                .join("\n"),
        ))
    })?;

    loop {
        if let Ok(messages) = scan_changes(http.clone(), &mut member_results, &mut contest) {
            for message in messages {
                channel
                    .send_message(&ctx, |e| {
                        e.content(format!("**{}**: {}", contest.name, message))
                    })
                    .ok();
            }
        }
        if contest.phase == ContestPhase::Finished {
            break;
        }
        // Sleep for a minute
        std::thread::sleep(std::time::Duration::from_secs(60));
    }

    // Announce the final results
    let mut ranks = member_results
        .into_iter()
        .filter_map(|(_, m)| {
            let member = m.member;
            let handle = m.handle;
            m.row.map(|row| ((handle, member), row))
        })
        .collect::<Vec<_>>();
    ranks.sort_by(|(_, a), (_, b)| a.rank.cmp(&b.rank));

    if ranks.is_empty() {
        channel.send_message(&ctx, |e| {
            e.content(format!(
                "**{}** has ended, but I can't find anyone in this server on the scoreboard...",
                contest.name
            ))
        })?;
        return Ok(());
    }

    channel.send_message(
        &ctx, |e|
        	e.content(format!(
            	"**{}** has ended, and the rankings in the server is:\n{}", contest.name,
            	ranks.into_iter().map(|((handle, mem), row)| format!(
                	"- **#{}**: {} (**{}**) with **{:.0}** points [{}] and ({} succeeded, {} failed) hacks!",
 			row.rank,
 			mem.mention(),
 			handle,
 			row.points,
 			row.problem_results.iter().map(|p| format!("{:.0}", p.points)).collect::<Vec<_>>().join("/"),
 			row.successful_hack_count,
 			row.unsuccessful_hack_count,
            	)).collect::<Vec<_>>().join("\n"))))?;

    Ok(())
}

fn scan_changes(
    http: <HTTPClient as TypeMapKey>::Value,
    members: &mut HashMap<UserId, MemberResult>,
    contest: &mut Contest,
) -> Result<Vec<String>, CommandError> {
    let mut messages: Vec<String> = vec![];
    let (updated_contest, problems, ranks) = {
        let handles = members
            .iter()
            .map(|(_, h)| h.handle.clone())
            .collect::<Vec<_>>();
        Contest::standings(&http, contest.id, |f| f.handles(handles))?
    };
    // Change of phase.
    if contest.phase != updated_contest.phase {
        messages.push(updated_contest.phase.to_string());
    }
    let mut handle_to_user_id = members
        .iter_mut()
        .map(|(_, v)| (v.handle.clone(), v))
        .collect::<HashMap<_, _>>();
    // Change of ProblemResult...
    for row in ranks {
        // Gotta find an user id?
        let user_ids = row
            .party
            .members
            .iter()
            .filter_map(|v| handle_to_user_id.get(&v.handle));
        // Scan for changes immutably
        for member_result in user_ids {
            let old_row = member_result.row.as_ref().cloned().unwrap_or(RanklistRow {
                party: row.party.clone(),
                rank: 0,
                points: 0.0,
                penalty: 0,
                successful_hack_count: 0,
                unsuccessful_hack_count: 0,
                problem_results: vec![
                    ProblemResult {
                        points: 0.0,
                        penalty: None,
                        rejected_attempt_count: 0,
                        result_type: ProblemResultType::Preliminary,
                        best_submission_time_seconds: None
                    };
                    row.problem_results.len()
                ],
                last_submission_time_seconds: None,
            });
            messages.extend(translate_overall_result(
                member_result.handle.as_str(),
                &old_row,
                &row,
                &member_result.member,
            ));
            for (problem, (old, new)) in problems.iter().zip(
                old_row
                    .problem_results
                    .iter()
                    .zip(row.problem_results.iter()),
            ) {
                if let Some(message) = analyze_change(&contest, old, new).map(|c| {
                    translate_change(
                        member_result.handle.as_str(),
                        &row,
                        &member_result.member,
                        problem,
                        new,
                        c,
                    )
                }) {
                    messages.push(message);
                }
            }
        }
        // Update list mutably
        for handle in row.party.members.iter().map(|v| v.handle.as_str()) {
            if let Some(mut u) = handle_to_user_id.get_mut(handle) {
                u.row = Some(row.clone());
            }
        }
    }

    // Update entire contest
    *contest = updated_contest;

    Ok(messages)
}

fn translate_overall_result(
    handle: &str,
    old_row: &RanklistRow,
    new_row: &RanklistRow,
    member: &Member,
) -> Vec<String> {
    let mention = || -> MessageBuilder {
        let mut m = MessageBuilder::new();
        m.push_bold_safe(handle)
            .push(" (")
            .push_safe(member.distinct())
            .push(")");
        m
    };
    let mut res = vec![];

    // Hack counts
    if new_row.successful_hack_count > old_row.successful_hack_count {
        res.push(
            mention()
                .push(format!(
                    " attempted a hack and **succeeded**! 🕵️ : **{:.0}** points placing #**{}**!",
                    new_row.points, new_row.rank
                ))
                .build(),
        );
    }
    if new_row.unsuccessful_hack_count > old_row.unsuccessful_hack_count {
        res.push(
            mention()
                .push(format!(
                    " attempted a hack but **did not succeed** 😣: **{}** points placing #**{}**.",
                    new_row.points, new_row.rank
                ))
                .build(),
        );
    }

    res
}

fn translate_change(
    handle: &str,
    row: &RanklistRow,
    member: &Member,
    problem: &Problem,
    res: &ProblemResult,
    change: Change,
) -> String {
    let mut m = MessageBuilder::new();
    m.push_bold_safe(handle)
        .push(" (")
        .push_safe(member.distinct())
        .push(")");

    use Change::*;
    match change {
        PretestsPassed => m
            .push(" passed the pretest on problem ")
            .push_bold_safe(format!("{} - {}", problem.index, problem.name))
            .push(", scoring ")
            .push_bold(format!("{:.0}", res.points))
            .push(" points, now at ")
            .push_bold(format!("#{}", row.rank))
            .push("! 🍷"),
        Attempted => m
            .push(format!(
                " made another attempt (total **{}**) on problem ",
                res.rejected_attempt_count
            ))
            .push_bold_safe(format!("{} - {}", problem.index, problem.name))
            .push(", but did not get the right answer. Keep going! 🏃‍♂️"),
        Hacked => m
            .push(" got **hacked** on problem ")
            .push_bold_safe(format!("{} - {}", problem.index, problem.name))
            .push(", now at rank ")
            .push_bold(format!("#{}", row.rank))
            .push("... Find your bug!🐛"),
        Accepted => m
            .push(" got **Accepted** on the **final tests** on problem ")
            .push_bold_safe(format!("{} - {}", problem.index, problem.name))
            .push(", permanently scoring ")
            .push_bold(format!("{:.0}", res.points))
            .push(" points, now at ")
            .push_bold(format!("#{}", row.rank))
            .push(" 🎉"),
        TestFailed => m
            .push(" **failed** on the **final tests** on problem ")
            .push_bold_safe(format!("{} - {}", problem.index, problem.name))
            .push(", now at ")
            .push_bold(format!("#{}", row.rank))
            .push(" 😭"),
    };
    m.build()
}

enum Change {
    PretestsPassed,
    Attempted,
    Hacked,
    Accepted,
    TestFailed,
}

fn analyze_change(contest: &Contest, old: &ProblemResult, new: &ProblemResult) -> Option<Change> {
    use Change::*;
    if old.points == new.points {
        if new.rejected_attempt_count > old.rejected_attempt_count {
            if new.result_type == ProblemResultType::Preliminary {
                return Some(Attempted);
            } else {
                return Some(TestFailed);
            }
        }
        if old.result_type != new.result_type && new.points > 0.0 {
            return Some(Accepted);
        }
        None
    } else {
        if new.points == 0.0 {
            if new.result_type == ProblemResultType::Preliminary {
                if contest.phase == ContestPhase::Coding {
                    Some(Hacked)
                } else {
                    None // Just changes to In Queue...
                }
            } else {
                Some(TestFailed)
            }
        } else if new.points > old.points {
            if new.result_type == ProblemResultType::Preliminary {
                Some(PretestsPassed)
            } else {
                Some(Accepted)
            }
        } else {
            Some(PretestsPassed)
        }
    }
}

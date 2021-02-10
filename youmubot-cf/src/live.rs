use crate::{db::CfSavedUsers, CFClient};
use codeforces::{Contest, ContestPhase, Problem, ProblemResult, ProblemResultType, RanklistRow};
use serenity::{
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
pub async fn watch_contest(
    ctx: &Context,
    guild: GuildId,
    channel: ChannelId,
    contest_id: u64,
) -> Result<()> {
    let data = ctx.data.read().await;
    let db = CfSavedUsers::open(&*data).borrow()?.clone();
    let member_cache = data.get::<member_cache::MemberCache>().unwrap().clone();
    let mut msg = channel
        .send_message(&ctx, |e| e.content("Youmu is building the member list..."))
        .await?;
    // Collect an initial member list.
    // This never changes during the scan.
    let mut member_results: HashMap<UserId, MemberResult> = db
        .into_iter()
        .map(|(user_id, cfu)| {
            let member_cache = &member_cache;
            async move {
                member_cache.query(ctx, user_id, guild).await.map(|m| {
                    (
                        user_id,
                        MemberResult {
                            member: m,
                            handle: cfu.handle,
                            row: None,
                        },
                    )
                })
            }
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| future::ready(v))
        .collect()
        .await;

    let http = data.get::<CFClient>().unwrap();
    let (mut contest, problems, _) =
        Contest::standings(&*http.borrow().await?, contest_id, |f| f.limit(1, 1)).await?;

    msg.edit(&ctx, |e| {
        e.content(format!(
            "Youmu is watching contest **{}**, with the following members: {}",
            contest.name,
            member_results
                .iter()
                .map(|(_, m)| serenity::utils::MessageBuilder::new()
                    .push_safe(m.member.distinct())
                    .push(" (")
                    .push_mono_safe(&m.handle)
                    .push(")")
                    .build())
                .collect::<Vec<_>>()
                .join(", "),
        ))
    })
    .await?;
    msg.pin(ctx).await.ok();

    loop {
        if let Ok(messages) =
            scan_changes(&*http.borrow().await?, &mut member_results, &mut contest).await
        {
            for message in messages {
                channel
                    .send_message(&ctx, |e| {
                        e.content(format!("**{}**: {}", contest.name, message))
                    })
                    .await
                    .ok();
            }
        }
        if contest.phase == ContestPhase::Finished {
            break;
        }
        // Sleep for a minute
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    // Announce the final results
    let mut ranks = member_results
        .into_iter()
        .filter_map(|(_, m)| {
            let member = m.member;
            let handle = m.handle;
            m.row.map(|row| (member, handle, row))
        })
        .collect::<Vec<_>>();
    ranks.sort_by(|(_, _, a), (_, _, b)| a.rank.cmp(&b.rank));

    msg.unpin(ctx).await.ok();
    crate::contest_rank_table(&ctx, &msg, contest, problems, ranks).await?;
    Ok(())
}

fn mention(phase: ContestPhase, m: &Member) -> String {
    match phase {
        ContestPhase::Before | ContestPhase::Coding =>
        // Don't mention directly, avoid spamming in contest
        {
            MessageBuilder::new().push_safe(m.distinct()).build()
        }
        _ => m.mention().to_string(),
    }
}

async fn scan_changes(
    http: &codeforces::Client,
    members: &mut HashMap<UserId, MemberResult>,
    contest: &mut Contest,
) -> Result<Vec<String>> {
    let mut messages: Vec<String> = vec![];
    let (updated_contest, problems, ranks) = {
        let handles = members
            .iter()
            .map(|(_, h)| h.handle.clone())
            .collect::<Vec<_>>();
        Contest::standings(&http, contest.id, |f| f.handles(handles)).await?
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
                contest.phase,
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
                        contest.phase,
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
    phase: ContestPhase,
    handle: &str,
    old_row: &RanklistRow,
    new_row: &RanklistRow,
    member: &Member,
) -> Vec<String> {
    let mention = || -> MessageBuilder {
        let mut m = MessageBuilder::new();
        m.push_bold_safe(handle)
            .push(" (")
            .push(mention(phase, member))
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
    phase: ContestPhase,
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
        .push_safe(mention(phase, member))
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

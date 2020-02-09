use codeforces::{Problem, ProblemResult, ProblemResultType, RanklistRow};
use serenity::{
    framework::standard::CommandResult,
    model::{
        guild::Member,
        id::{ChannelId, GuildId},
    },
    utils::MessageBuilder,
};
use youmubot_prelude::*;

/// Watch and commentate a contest.
///
/// Does the thing on a channel, block until the contest ends.
pub fn watch_contest(
    ctx: &mut Context,
    guild: GuildId,
    channel: ChannelId,
    contest_id: u64,
) -> CommandResult {
    unimplemented!()
}

fn scan_changes(
    http: <HTTPClient as TypeMapKey>::Value,
    members: &[(Member, &str)],
) -> Vec<String> {
    unimplemented!()
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

fn analyze_change(old: &ProblemResult, new: &ProblemResult) -> Option<Change> {
    use Change::*;
    if old.points == new.points {
        if new.rejected_attempt_count > old.rejected_attempt_count {
            return Some(Attempted);
        }
        if old.result_type != new.result_type {
            return Some(Accepted);
        }
        None
    } else {
        if new.points == 0.0 {
            if new.result_type == ProblemResultType::Preliminary {
                Some(Hacked)
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

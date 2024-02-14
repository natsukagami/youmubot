use std::{collections::HashMap, str::FromStr};

use super::{
    db::{OsuSavedUsers, OsuUserBests},
    ModeArg, OsuClient,
};
use crate::{
    discord::{
        display::ScoreListStyle,
        oppai_cache::{Accuracy, BeatmapCache},
        BeatmapWithMode,
    },
    models::{Mode, Mods, Score},
    request::UserID,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::Message, id::UserId},
    utils::MessageBuilder,
};
use youmubot_prelude::*;

#[derive(Debug, Clone, Copy)]
enum ModeOrTotal {
    Total,
    Mode(Mode),
}

impl FromStr for ModeOrTotal {
    type Err = <ModeArg as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "total" {
            Ok(ModeOrTotal::Total)
        } else {
            ModeArg::from_str(s).map(|ModeArg(m)| ModeOrTotal::Mode(m))
        }
    }
}

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let mode = args
        .single::<ModeOrTotal>()
        .unwrap_or(ModeOrTotal::Mode(Mode::Std));
    let guild = m.guild_id.expect("Guild-only command");
    // let member_cache = data.get::<MemberCache>().unwrap();
    let osu_users = data
        .get::<OsuSavedUsers>()
        .unwrap()
        .all()
        .await?
        .into_iter()
        .map(|v| (v.user_id, v))
        .collect::<HashMap<_, _>>();
    let users = guild
        .members_iter(ctx)
        .filter_map(|m| {
            future::ready(
                m.ok()
                    .and_then(|m| osu_users.get(&m.user.id).map(|ou| (m, ou))),
            )
        })
        .filter_map(|(member, osu_user)| {
            future::ready(|| -> Option<_> {
                let pp = match mode {
                    ModeOrTotal::Total
                        if osu_user.pp.iter().any(|v| v.is_some_and(|v| v > 0.0)) =>
                    {
                        Some(osu_user.pp.iter().map(|v| v.unwrap_or(0.0)).sum())
                    }
                    ModeOrTotal::Mode(m) => osu_user.pp.get(m as usize).and_then(|v| *v),
                    _ => None,
                }?;
                Some((pp, member.user.name, osu_user))
            }())
        })
        .collect::<Vec<_>>()
        .await;
    let last_update = users.iter().map(|(_, _, a)| a.last_update).min();
    let mut users = users
        .into_iter()
        .map(|(a, b, u)| (a, (b, u.clone())))
        .collect::<Vec<_>>();
    users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")
            .await?;
        return Ok(());
    }

    let users = std::sync::Arc::new(users);
    let last_update = last_update.unwrap();
    paginate_reply_fn(
        move |page: u8, ctx: &Context, m: &mut Message| {
            const ITEMS_PER_PAGE: usize = 10;
            let users = users.clone();
            Box::pin(async move {
                let start = (page as usize) * ITEMS_PER_PAGE;
                let end = (start + ITEMS_PER_PAGE).min(users.len());
                if start >= end {
                    return Ok(false);
                }
                let total_len = users.len();
                let users = &users[start..end];
                let username_len = users
                    .iter()
                    .map(|(_, (_, u))| u.username.len())
                    .max()
                    .unwrap_or(8)
                    .max(8);
                let member_len = users
                    .iter()
                    .map(|(_, (mem, _))| mem.len())
                    .max()
                    .unwrap_or(8)
                    .max(8);
                let mut content = MessageBuilder::new();
                content
                    .push_line("```")
                    .push_line(format!(
                        "Rank | pp       | {:uw$} | Member",
                        "Username",
                        uw = username_len
                    ))
                    .push_line(format!(
                        "------------------{:-<uw$}---{:-<mw$}",
                        "",
                        "",
                        uw = username_len,
                        mw = member_len
                    ));
                for (id, (pp, (member, u))) in users.iter().enumerate() {
                    content.push_line(format!(
                        "{:>4} | {:>8.2} | {:uw$} | {}",
                        format!("#{}", 1 + id + start),
                        pp,
                        u.username,
                        member,
                        uw = username_len
                    ));
                }
                content.push_line("```").push_line(format!(
                    "Page **{}**/**{}**. Last updated: {}",
                    page + 1,
                    (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                    last_update.format("<t:%s:R>"),
                ));
                m.edit(ctx, |f| f.content(content.to_string())).await?;
                Ok(true)
            })
        },
        ctx,
        m,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

pub(crate) mod update_lock {
    use serenity::{model::id::GuildId, prelude::TypeMapKey};
    use std::collections::HashSet;
    use std::sync::Mutex;
    #[derive(Debug, Default)]
    pub struct UpdateLock(Mutex<HashSet<GuildId>>);

    pub struct UpdateLockGuard<'a>(&'a UpdateLock, GuildId);

    impl TypeMapKey for UpdateLock {
        type Value = UpdateLock;
    }

    impl UpdateLock {
        pub fn get(&self, guild: GuildId) -> Option<UpdateLockGuard> {
            let mut set = self.0.lock().unwrap();
            if set.contains(&guild) {
                None
            } else {
                set.insert(guild);
                Some(UpdateLockGuard(self, guild))
            }
        }
    }

    impl<'a> Drop for UpdateLockGuard<'a> {
        fn drop(&mut self) {
            let mut set = self.0 .0.lock().unwrap();
            set.remove(&self.1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderBy {
    PP,
    Score,
}

impl Default for OrderBy {
    fn default() -> Self {
        Self::PP
    }
}

impl std::str::FromStr for OrderBy {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--score" => Ok(OrderBy::Score),
            "--pp" => Ok(OrderBy::PP),
            _ => Err(Error::msg("unknown value")),
        }
    }
}

#[command("leaderboard")]
#[aliases("lb", "bmranks", "br", "cc", "updatelb")]
#[usage = "[--score to sort by score, default to sort by pp] / [--table to show a table, --grid to show score by score] / [mods to filter]"]
#[description = "See the server's ranks on the last seen beatmap"]
#[max_args(2)]
#[only_in(guilds)]
pub async fn update_leaderboard(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let sort_order = args.single::<OrderBy>().unwrap_or_default();
    let style = args.single::<ScoreListStyle>().unwrap_or_default();

    let guild = m.guild_id.unwrap();
    let data = ctx.data.read().await;
    let update_lock = data.get::<update_lock::UpdateLock>().unwrap();
    let update_lock = match update_lock.get(guild) {
        None => {
            m.reply(&ctx, "Another update is running.").await?;
            return Ok(());
        }
        Some(v) => v,
    };
    let (bm, mods) = match super::load_beatmap(ctx, m).await {
        Some((bm, mods_def)) => {
            let mods = args.find::<Mods>().ok().or(mods_def).unwrap_or(Mods::NOMOD);
            (bm, mods)
        }
        None => {
            m.reply(&ctx, "No beatmap queried on this channel.").await?;
            return Ok(());
        }
    };
    let mode = bm.1;
    let member_cache = data.get::<MemberCache>().unwrap();
    // Signal that we are running.
    let running_reaction = m.react(&ctx, '⌛').await?;

    // Run a check on everyone in the server basically.
    let all_server_users: Vec<(UserId, Vec<Score>)> = {
        let osu = data.get::<OsuClient>().unwrap();
        let osu_users = data
            .get::<OsuSavedUsers>()
            .unwrap()
            .all()
            .await?
            .into_iter()
            .map(|osu_user| (osu_user.user_id, osu_user.id));
        let beatmap_id = bm.0.beatmap_id;
        osu_users
            .map(|(user_id, osu_id)| {
                member_cache
                    .query(&ctx, user_id, guild)
                    .map(move |t| t.map(|_| (user_id, osu_id)))
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(future::ready)
            .filter_map(|(member, osu_id)| async move {
                let scores = osu
                    .scores(beatmap_id, |f| f.user(UserID::ID(osu_id)).mode(mode))
                    .await
                    .ok();
                scores
                    .filter(|s| !s.is_empty())
                    .map(|scores| (member, scores))
            })
            .collect::<Vec<_>>()
            .await
    };
    let updated_users = all_server_users.len();
    // Update everything.
    {
        let db = data.get::<OsuUserBests>().unwrap();
        all_server_users
            .into_iter()
            .map(|(u, scores)| db.save(u, mode, scores))
            .collect::<stream::FuturesUnordered<_>>()
            .try_collect::<()>()
            .await?;
    }
    // Signal update complete.
    running_reaction.delete(&ctx).await.ok();
    m.reply(
        &ctx,
        format!(
            "update for beatmap (`{}`) complete, {} users updated.",
            bm.0.short_link(if bm.mode() != bm.1 { Some(bm.1) } else { None }, None),
            updated_users
        ),
    )
    .await
    .ok();
    drop(update_lock);
    show_leaderboard(ctx, m, bm, mods, sort_order, style).await
}

async fn show_leaderboard(
    ctx: &Context,
    m: &Message,
    bm: BeatmapWithMode,
    mods: Mods,
    order: OrderBy,
    style: ScoreListStyle,
) -> CommandResult {
    let data = ctx.data.read().await;

    // Get oppai map.
    let mode = bm.1;
    let oppai = data.get::<BeatmapCache>().unwrap();
    let oppai_map = oppai.get_beatmap(bm.0.beatmap_id).await?;
    let get_oppai_pp = move |combo: u64, acc: Accuracy, mods: Mods| {
        oppai_map
            .get_pp_from(mode, Some(combo as usize), acc, mods)
            .ok()
    };

    let guild = m.guild_id.expect("Guild-only command");
    let member_cache = data.get::<MemberCache>().unwrap();
    let scores = {
        const NO_SCORES: &str = "No scores have been recorded for this beatmap.";

        let scores = data
            .get::<OsuUserBests>()
            .unwrap()
            .by_beatmap(bm.0.beatmap_id, bm.1)
            .await?;
        if scores.is_empty() {
            m.reply(&ctx, NO_SCORES).await?;
            return Ok(());
        }

        let mut scores: Vec<(f64, String, Score)> = scores
            .into_iter()
            .filter(|(_, score)| score.mods.contains(mods))
            .map(|(user_id, score)| {
                member_cache
                    .query(&ctx, user_id, guild)
                    .map(|m| m.map(move |m| (m.distinct(), score)))
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(future::ready)
            .filter_map(|(user, score)| {
                future::ready(
                    score
                        .pp
                        .or_else(|| {
                            get_oppai_pp(
                                score.max_combo,
                                Accuracy::ByCount(
                                    score.count_300,
                                    score.count_100,
                                    score.count_50,
                                    score.count_miss,
                                ),
                                score.mods,
                            )
                        })
                        .map(|v| (v, user, score)),
                )
            })
            .collect::<Vec<_>>()
            .await;
        match order {
            OrderBy::PP => scores.sort_by(|(a, _, _), (b, _, _)| {
                b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
            }),
            OrderBy::Score => {
                scores.sort_by(|(_, _, a), (_, _, b)| b.normalized_score.cmp(&a.normalized_score))
            }
        };
        scores
    };

    if scores.is_empty() {
        m.reply(
            &ctx,
            "No scores have been recorded for this beatmap. Run `osu check` to scan for yours!",
        )
        .await?;
        return Ok(());
    }

    if let ScoreListStyle::Grid = style {
        style
            .display_scores(
                scores.into_iter().map(|(_, _, a)| a).collect(),
                mode,
                ctx,
                m,
            )
            .await?;
        return Ok(());
    }
    let has_lazer_score = scores.iter().any(|(_, _, v)| v.score.is_none());

    paginate_reply_fn(
        move |page: u8, ctx: &Context, m: &mut Message| {
            const ITEMS_PER_PAGE: usize = 5;
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return Box::pin(future::ready(Ok(false)));
            }
            let total_len = scores.len();
            let scores = scores[start..end].to_vec();
            let bm = (bm.0.clone(), bm.1);
            Box::pin(async move {
                // username width
                let uw = scores
                    .iter()
                    .map(|(_, u, _)| u.len())
                    .max()
                    .unwrap_or(8)
                    .max(8);
                let accuracies = scores
                    .iter()
                    .map(|(_, _, v)| format!("{:.2}%", v.accuracy(bm.1)))
                    .collect::<Vec<_>>();
                let aw = accuracies.iter().map(|v| v.len()).max().unwrap().max(3);
                let misses = scores
                    .iter()
                    .map(|(_, _, v)| format!("{}", v.count_miss))
                    .collect::<Vec<_>>();
                let mw = misses.iter().map(|v| v.len()).max().unwrap().max(4);
                let ranks = scores
                    .iter()
                    .map(|(_, _, v)| v.rank.to_string())
                    .collect::<Vec<_>>();
                let rw = ranks.iter().map(|v| v.len()).max().unwrap().max(4);
                let pp_label = match order {
                    OrderBy::PP => "pp",
                    OrderBy::Score => "score",
                };
                let pp = scores
                    .iter()
                    .map(|(pp, _, s)| match order {
                        OrderBy::PP => format!("{:.2}", pp),
                        OrderBy::Score => crate::discord::embeds::grouped_number(if has_lazer_score { s.normalized_score as u64 } else { s.score.unwrap() }),
                    })
                    .collect::<Vec<_>>();
                let pw = pp.iter().map(|v| v.len()).max().unwrap_or(pp_label.len());
                /*mods width*/
                let mdw = scores
                    .iter()
                    .map(|(_, _, v)| v.mods.str_len())
                    .max()
                    .unwrap()
                    .max(4);
                let combos = scores
                    .iter()
                    .map(|(_, _, v)| format!("{}x", v.max_combo))
                    .collect::<Vec<_>>();
                let cw = combos
                    .iter()
                    .map(|v| v.len())
                    .max()
                    .unwrap()
                    .max(5);
                let mut content = MessageBuilder::new();
                content
                    .push_line("```")
                    .push_line(format!(
                        "rank | {:>pw$} | {:mdw$} | {:rw$} | {:>aw$} | {:>cw$} | {:mw$} | {:uw$}",
                        pp_label,
                        "mods",
                        "rank",
                        "acc",
                        "combo",
                        "miss",
                        "user",
                        pw = pw,
                        mdw = mdw,
                        rw = rw,
                        aw = aw,
                        mw = mw,
                        uw = uw,
                        cw = cw,
                    ))
                    .push_line(format!(
                        "-------{:-<pw$}---{:-<mdw$}---{:-<rw$}---{:-<aw$}---{:-<cw$}---{:-<mw$}---{:-<uw$}",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        pw = pw,
                        mdw = mdw,
                        rw = rw,
                        aw = aw,
                        mw = mw,
                        uw = uw,
                        cw = cw,
                    ));
                for (id, (_, member, p)) in scores.iter().enumerate() {
                    content.push_line_safe(format!(
                        "{:>4} | {:>pw$} | {} | {:>rw$} | {:>aw$} | {:>cw$} | {:>mw$} | {:uw$}",
                        format!("#{}", 1 + id + start),
                        pp[id],
                        p.mods.to_string_padded(mdw),
                        ranks[id],
                        accuracies[id],
                        combos[id],
                        misses[id],
                        member,
                        pw = pw,
                        rw = rw,
                        aw = aw,
                        cw = cw,
                        mw = mw,
                        uw = uw,
                    ));
                }
                content.push_line("```").push_line(format!(
                    "Page **{}**/**{}**. Not seeing your scores? Run `osu check` to update.",
                    page + 1,
                    (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                ));
                if let crate::models::ApprovalStatus::Ranked(_) = bm.0.approval {
                } else if order == OrderBy::PP {
                    content.push_line("PP was calculated by `oppai-rs`, **not** official values.");
                }

                m.edit(&ctx, |f| f.content(content.build())).await?;
                Ok(true)
            })
        },
        ctx,
        m,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

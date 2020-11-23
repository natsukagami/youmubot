use super::{
    cache::get_beatmap,
    db::{OsuSavedUsers, OsuUserBests},
    ModeArg, OsuClient,
};
use crate::{
    discord::BeatmapWithMode,
    models::{Mode, Score},
    request::UserID,
};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::Message, id::UserId},
    utils::MessageBuilder,
};
use youmubot_prelude::*;

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let mode = args.single::<ModeArg>().map(|v| v.0).unwrap_or(Mode::Std);
    let guild = m.guild_id.expect("Guild-only command");
    let member_cache = data.get::<MemberCache>().unwrap();
    let users = OsuSavedUsers::open(&*data).borrow()?.clone();
    let users = users
        .into_iter()
        .map(|(user_id, osu_user)| async move {
            member_cache
                .query(&ctx, user_id, guild)
                .await
                .and_then(|member| {
                    osu_user
                        .pp
                        .get(mode as usize)
                        .cloned()
                        .and_then(|pp| pp)
                        .map(|pp| (pp, member.distinct(), osu_user.last_update.clone()))
                })
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| future::ready(v))
        .collect::<Vec<_>>()
        .await;
    let last_update = users.iter().map(|(_, _, a)| a).min().cloned();
    let mut users = users
        .into_iter()
        .map(|(a, b, _)| (a, b))
        .collect::<Vec<_>>();
    users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")
            .await?;
        return Ok(());
    }

    let users = std::sync::Arc::new(users);
    let last_update = last_update.unwrap();
    paginate_fn(
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
                let username_len = users.iter().map(|(_, u)| u.len()).max().unwrap_or(8).max(8);
                let mut content = MessageBuilder::new();
                content
                    .push_line("```")
                    .push_line("Rank | pp      | Username")
                    .push_line(format!("-----------------{:-<uw$}", "", uw = username_len));
                for (id, (pp, member)) in users.iter().enumerate() {
                    content
                        .push(format!(
                            "{:>4} | {:>7.2} | ",
                            format!("#{}", 1 + id + start),
                            pp
                        ))
                        .push_line_safe(member);
                }
                content.push_line("```").push_line(format!(
                    "Page **{}**/**{}**. Last updated: `{}`",
                    page + 1,
                    (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                    last_update.to_rfc2822()
                ));
                m.edit(ctx, |f| f.content(content.to_string())).await?;
                Ok(true)
            })
        },
        ctx,
        m.channel_id,
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

#[command("updatelb")]
#[description = "Update the leaderboard on the last seen beatmap"]
#[max_args(0)]
#[only_in(guilds)]
pub async fn update_leaderboard(ctx: &Context, m: &Message, mut _args: Args) -> CommandResult {
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
    let bm = match get_beatmap(&*data, m.channel_id)? {
        Some(bm) => bm,
        None => {
            m.reply(&ctx, "No beatmap queried on this channel.").await?;
            return Ok(());
        }
    };
    let member_cache = data.get::<MemberCache>().unwrap();
    // Signal that we are running.
    let running_reaction = m.react(&ctx, '⌛').await?;

    // Run a check on everyone in the server basically.
    let all_server_users: Vec<(UserId, Vec<Score>)> = {
        let osu = data.get::<OsuClient>().unwrap();
        let osu_users = OsuSavedUsers::open(&*data);
        let osu_users = osu_users
            .borrow()?
            .iter()
            .map(|(&user_id, osu_user)| (user_id, osu_user.id))
            .collect::<Vec<_>>();
        let beatmap_id = bm.0.beatmap_id;
        osu_users
            .into_iter()
            .map(|(user_id, osu_id)| {
                member_cache
                    .query(&ctx, user_id, guild)
                    .map(move |t| t.map(|_| (user_id, osu_id)))
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(future::ready)
            .filter_map(|(member, osu_id)| async move {
                let scores = osu
                    .scores(beatmap_id, |f| f.user(UserID::ID(osu_id)))
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
        let mut osu_user_bests = OsuUserBests::open(&*data);
        let mut osu_user_bests = osu_user_bests.borrow_mut()?;
        let user_bests = osu_user_bests.entry((bm.0.beatmap_id, bm.1)).or_default();
        all_server_users.into_iter().for_each(|(member, scores)| {
            user_bests.insert(member, scores);
        })
    }
    // Signal update complete.
    running_reaction.delete(&ctx).await.ok();
    m.reply(
        &ctx,
        format!(
            "update for beatmap ({}, {}) complete, {} users updated.",
            bm.0.beatmap_id, bm.1, updated_users
        ),
    )
    .await
    .ok();
    drop(update_lock);
    show_leaderboard(ctx, m, bm).await
}

#[command("leaderboard")]
#[aliases("lb", "bmranks", "br", "cc")]
#[description = "See the server's ranks on the last seen beatmap"]
#[max_args(0)]
#[only_in(guilds)]
pub async fn leaderboard(ctx: &Context, m: &Message, mut _args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let bm = match get_beatmap(&*data, m.channel_id)? {
        Some(bm) => bm,
        None => {
            m.reply(&ctx, "No beatmap queried on this channel.").await?;
            return Ok(());
        }
    };
    show_leaderboard(ctx, m, bm).await
}

async fn show_leaderboard(ctx: &Context, m: &Message, bm: BeatmapWithMode) -> CommandResult {
    let data = ctx.data.read().await;
    let mut osu_user_bests = OsuUserBests::open(&*data);

    // Run a check on the user once too!
    {
        let osu_users = OsuSavedUsers::open(&*data);
        let user = osu_users.borrow()?.get(&m.author.id).map(|v| v.id);
        if let Some(id) = user {
            let osu = data.get::<OsuClient>().unwrap();
            if let Ok(scores) = osu
                .scores(bm.0.beatmap_id, |f| f.user(UserID::ID(id)))
                .await
            {
                if !scores.is_empty() {
                    osu_user_bests
                        .borrow_mut()?
                        .entry((bm.0.beatmap_id, bm.1))
                        .or_default()
                        .insert(m.author.id, scores);
                }
            }
        }
    }

    let guild = m.guild_id.expect("Guild-only command");
    let member_cache = data.get::<MemberCache>().unwrap();
    let scores = {
        const NO_SCORES: &'static str =
            "No scores have been recorded for this beatmap. Run `osu check` to scan for yours!";

        let users = osu_user_bests
            .borrow()?
            .get(&(bm.0.beatmap_id, bm.1))
            .cloned();
        let users = match users {
            None => {
                m.reply(&ctx, NO_SCORES).await?;
                return Ok(());
            }
            Some(v) if v.is_empty() => {
                m.reply(&ctx, NO_SCORES).await?;
                return Ok(());
            }
            Some(v) => v,
        };

        let mut scores: Vec<(f64, String, Score)> = users
            .into_iter()
            .map(|(user_id, scores)| {
                member_cache
                    .query(&ctx, user_id, guild)
                    .map(|m| m.map(move |m| (m.distinct(), scores)))
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(|v| future::ready(v))
            .flat_map(|(user, scores)| {
                scores
                    .into_iter()
                    .map(move |v| future::ready((user.clone(), v.clone())))
                    .collect::<stream::FuturesUnordered<_>>()
            })
            .filter_map(|(user, score)| future::ready(score.pp.map(|v| (v, user, score))))
            .collect::<Vec<_>>()
            .await;
        scores
            .sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
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
    paginate_fn(
        move |page: u8, ctx: &Context, m: &mut Message| {
            const ITEMS_PER_PAGE: usize = 5;
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return Box::pin(future::ready(Ok(false)));
            }
            let total_len = scores.len();
            let scores = (&scores[start..end]).iter().cloned().collect::<Vec<_>>();
            let bm = (bm.0.clone(), bm.1.clone());
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
                let pp = scores
                    .iter()
                    .map(|(pp, _, _)| format!("{:.2}", pp))
                    .collect::<Vec<_>>();
                let pw = pp.iter().map(|v| v.len()).max().unwrap_or(2);
                /*mods width*/
                let mdw = scores
                    .iter()
                    .map(|(_, _, v)| v.mods.to_string().len())
                    .max()
                    .unwrap()
                    .max(4);
                let mut content = MessageBuilder::new();
                content
                    .push_line("```")
                    .push_line(format!(
                        "rank | {:>pw$} | {:mdw$} | {:rw$} | {:>aw$} | {:mw$} | {:uw$}",
                        "pp",
                        "mods",
                        "rank",
                        "acc",
                        "miss",
                        "user",
                        pw = pw,
                        mdw = mdw,
                        rw = rw,
                        aw = aw,
                        mw = mw,
                        uw = uw,
                    ))
                    .push_line(format!(
                        "-------{:-<pw$}---{:-<mdw$}---{:-<rw$}---{:-<aw$}---{:-<mw$}---{:-<uw$}",
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
                    ));
                for (id, (_, member, p)) in scores.iter().enumerate() {
                    content.push_line_safe(format!(
                        "{:>4} | {:>pw$} | {:>mdw$} | {:>rw$} | {:>aw$} | {:>mw$} | {:uw$}",
                        format!("#{}", 1 + id + start),
                        pp[id],
                        p.mods.to_string(),
                        ranks[id],
                        accuracies[id],
                        misses[id],
                        member,
                        pw = pw,
                        mdw = mdw,
                        rw = rw,
                        aw = aw,
                        mw = mw,
                        uw = uw,
                    ));
                }
                content.push_line("```").push_line(format!(
                    "Page **{}**/**{}**. Not seeing your scores? Run `osu check` to update.",
                    page + 1,
                    (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                ));
                m.edit(&ctx, |f| f.content(content.build())).await?;
                Ok(true)
            })
        },
        ctx,
        m.channel_id,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

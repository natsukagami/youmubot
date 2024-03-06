use std::{collections::HashMap, str::FromStr, sync::Arc};

use super::{db::OsuSavedUsers, ModeArg, OsuClient};
use crate::{
    discord::{
        display::ScoreListStyle,
        oppai_cache::{Accuracy, BeatmapCache},
    },
    models::{Mode, Mods},
    request::UserID,
};

use poise::CreateReply;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use youmubot_prelude::{stream::FuturesUnordered, *};

#[derive(Debug, Clone, Copy)]
enum RankQuery {
    Total,
    MapLength,
    Mode(Mode),
}

impl FromStr for RankQuery {
    type Err = <ModeArg as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "total" => Ok(RankQuery::Total),
            "map-length" => Ok(RankQuery::MapLength),
            _ => ModeArg::from_str(s).map(|ModeArg(m)| RankQuery::Mode(m)),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Align {
    Left,
    Middle,
    Right,
}

impl Align {
    fn pad(self, input: &str, len: usize) -> String {
        match self {
            Align::Left => format!("{:<len$}", input),
            Align::Middle => format!("{:^len$}", input),
            Align::Right => format!("{:>len$}", input),
        }
    }
}

fn table_formatting<const N: usize, S: AsRef<str> + std::fmt::Debug, Ts: AsRef<[[S; N]]>>(
    headers: &[&'static str; N],
    padding: &[Align; N],
    table: Ts,
) -> String {
    let table = table.as_ref();
    // get length for each column
    let lens = headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            table
                .iter()
                .map(|r| r.as_ref()[i].as_ref().len())
                .max()
                .unwrap_or(0)
                .max(header.len())
        })
        .collect::<Vec<_>>();
    // paint with message builder
    let mut m = MessageBuilder::new();
    m.push_line("```");
    // headers first
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            m.push(" | ");
        }
        m.push(padding[i].pad(header, lens[i]));
    }
    m.push_line("");
    // separator
    m.push_line(format!(
        "{:-<total$}",
        "",
        total = lens.iter().sum::<usize>() + (lens.len() - 1) * 3
    ));
    // table itself
    for row in table {
        let row = row.as_ref();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                m.push(" | ");
            }
            let cell = cell.as_ref();
            m.push(padding[i].pad(cell, lens[i]));
        }
        m.push_line("");
    }
    m.push("```");
    m.build()
}

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let mode = args
        .single::<RankQuery>()
        .unwrap_or(RankQuery::Mode(Mode::Std));
    let guild = m.guild_id.expect("Guild-only command");
    let member_cache = data.get::<MemberCache>().unwrap();
    let osu_users = data
        .get::<OsuSavedUsers>()
        .unwrap()
        .all()
        .await?
        .into_iter()
        .map(|v| (v.user_id, v))
        .collect::<HashMap<_, _>>();
    let users = member_cache
        .query_members(&ctx, guild)
        .await?
        .iter()
        .filter_map(|m| osu_users.get(&m.user.id).map(|ou| (m, ou)))
        .filter_map(|(member, osu_user)| {
            let pp = match mode {
                RankQuery::Total if osu_user.pp.iter().any(|v| v.is_some_and(|v| v > 0.0)) => {
                    Some(osu_user.pp.iter().map(|v| v.unwrap_or(0.0)).sum())
                }
                RankQuery::MapLength => osu_user.pp.get(Mode::Std as usize).and_then(|v| *v),
                RankQuery::Mode(m) => osu_user.pp.get(m as usize).and_then(|v| *v),
                _ => None,
            }?;
            Some((pp, member.user.name.clone(), osu_user))
        })
        .collect::<Vec<_>>();
    let last_update = users.iter().map(|(_, _, a)| a.last_update).min();
    let mut users = users
        .into_iter()
        .map(|(a, b, u)| (a, (b, u.clone())))
        .collect::<Vec<_>>();
    if matches!(mode, RankQuery::MapLength) {
        users.sort_by(|(_, (_, a)), (_, (_, b))| {
            (b.std_weighted_map_length)
                .partial_cmp(&a.std_weighted_map_length)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    }

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")
            .await?;
        return Ok(());
    }

    let users = std::sync::Arc::new(users);
    let last_update = last_update.unwrap();
    paginate_reply_fn(
        move |page: u8, _| {
            use Align::*;
            const ITEMS_PER_PAGE: usize = 10;
            let users = users.clone();
            Box::pin(async move {
                let start = (page as usize) * ITEMS_PER_PAGE;
                let end = (start + ITEMS_PER_PAGE).min(users.len());
                if start >= end {
                    return Ok(None);
                }
                let total_len = users.len();
                let users = &users[start..end];
                let table = if matches!(mode, RankQuery::Mode(Mode::Std) | RankQuery::MapLength) {
                    const HEADERS: [&'static str; 5] =
                        ["#", "pp", "Map length", "Username", "Member"];
                    const ALIGNS: [Align; 5] = [Right, Right, Right, Left, Left];

                    let table = users
                        .iter()
                        .enumerate()
                        .map(|(i, (pp, (mem, ou)))| {
                            let map_length = match ou.std_weighted_map_length {
                                Some(len) => {
                                    let trunc_secs = len.floor() as u64;
                                    let minutes = trunc_secs / 60;
                                    let seconds = len - (60 * minutes) as f64;
                                    format!("{}m{:05.2}s", minutes, seconds)
                                }
                                None => "unknown".to_owned(),
                            };
                            [
                                format!("{}", 1 + i + start),
                                format!("{:.2}", pp),
                                map_length,
                                ou.username.clone().into_owned(),
                                mem.clone(),
                            ]
                        })
                        .collect::<Vec<_>>();
                    table_formatting(&HEADERS, &ALIGNS, table)
                } else {
                    const HEADERS: [&'static str; 4] = ["#", "pp", "Username", "Member"];
                    const ALIGNS: [Align; 4] = [Right, Right, Left, Left];

                    let table = users
                        .iter()
                        .enumerate()
                        .map(|(i, (pp, (mem, ou)))| {
                            [
                                format!("{}", 1 + i + start),
                                format!("{:.2}", pp),
                                ou.username.clone().into_owned(),
                                mem.clone(),
                            ]
                        })
                        .collect::<Vec<_>>();
                    table_formatting(&HEADERS, &ALIGNS, table)
                };
                let content = MessageBuilder::new()
                    .push_line(table)
                    .push_line(format!(
                        "Page **{}**/**{}**. Last updated: {}",
                        page + 1,
                        (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                        last_update.format("<t:%s:R>"),
                    ))
                    .build();
                Ok(Some(CreateReply::default().content(content.to_string())))
            })
        },
        ctx,
        m.clone(),
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
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
pub async fn show_leaderboard(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let order = args.single::<OrderBy>().unwrap_or_default();
    let style = args.single::<ScoreListStyle>().unwrap_or_default();

    let data = ctx.data.read().await;
    let env = data.get::<crate::discord::Env>().unwrap();
    let member_cache = data.get::<MemberCache>().unwrap();

    let (bm, _) = match super::load_beatmap(env, Some(m), m.channel_id).await {
        Some((bm, mods_def)) => {
            let mods = args.find::<Mods>().ok().or(mods_def).unwrap_or(Mods::NOMOD);
            (bm, mods)
        }
        None => {
            m.reply(&ctx, "No beatmap queried on this channel.").await?;
            return Ok(());
        }
    };

    let osu = data.get::<OsuClient>().unwrap().clone();

    // Get oppai map.
    let mode = bm.1;
    let oppai = data.get::<BeatmapCache>().unwrap();
    let oppai_map = oppai.get_beatmap(bm.0.beatmap_id).await?;

    let guild = m.guild_id.expect("Guild-only command");
    let scores = {
        const NO_SCORES: &str = "No scores have been recorded for this beatmap.";
        // Signal that we are running.
        let running_reaction = m.react(&ctx, '⌛').await?;

        let osu_users = data
            .get::<OsuSavedUsers>()
            .unwrap()
            .all()
            .await?
            .into_iter()
            .map(|v| (v.user_id, v))
            .collect::<HashMap<_, _>>();
        let mut scores = member_cache
            .query_members(&ctx, guild)
            .await?
            .iter()
            .filter_map(|m| osu_users.get(&m.user.id).map(|ou| (m.distinct(), ou.id)))
            .map(|(mem, osu_id)| {
                osu.scores(bm.0.beatmap_id, move |f| {
                    f.user(UserID::ID(osu_id)).mode(bm.1)
                })
                .map(|r| Some((mem, r.ok()?)))
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(future::ready)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flat_map(|(mem, scores)| {
                let mem = Arc::new(mem);
                scores
                    .into_iter()
                    .filter_map(|score| {
                        let pp = score.pp.map(|v| (true, v)).or_else(|| {
                            oppai_map
                                .get_pp_from(
                                    mode,
                                    Some(score.max_combo as usize),
                                    Accuracy::ByCount(
                                        score.count_300,
                                        score.count_100,
                                        score.count_50,
                                        score.count_miss,
                                    ),
                                    score.mods,
                                )
                                .ok()
                                .map(|v| (false, v))
                        })?;
                        Some((pp, mem.clone(), score))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        running_reaction.delete(&ctx).await?;

        if scores.is_empty() {
            m.reply(&ctx, NO_SCORES).await?;
            return Ok(());
        }
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
        crate::discord::display::scores::display_scores(
            style,
            scores.into_iter().map(|(_, _, a)| a).collect(),
            mode,
            ctx,
            m.clone(),
            m.channel_id,
        )
        .await?;
        return Ok(());
    }
    let has_lazer_score = scores.iter().any(|(_, _, v)| v.score.is_none());

    paginate_reply_fn(
        move |page: u8, ctx: &Context| {
            const ITEMS_PER_PAGE: usize = 5;
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return Box::pin(future::ready(Ok(None)));
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
                    .map(|((official, pp), _, s)| match order {
                        OrderBy::PP => format!("{:.2}{}", pp, if *official { "" } else { "[?]" }),
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

                Ok(Some(CreateReply::default().content(content.build())))
            })
        },
        ctx,
        m.clone(),
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

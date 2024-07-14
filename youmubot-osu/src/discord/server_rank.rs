use std::{collections::HashMap, str::FromStr, sync::Arc};

use pagination::paginate_with_first_message;
use serenity::{
    all::GuildId,
    builder::EditMessage,
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};

use youmubot_prelude::table_format::Align::{Left, Right};
use youmubot_prelude::{
    stream::FuturesUnordered,
    table_format::{table_formatting, Align},
    *,
};

use crate::{
    discord::{display::ScoreListStyle, oppai_cache::Accuracy, BeatmapWithMode},
    models::{Mode, Mods},
    request::UserID,
    Score,
};

use super::{ModeArg, OsuEnv};

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

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let mode = args
        .single::<RankQuery>()
        .unwrap_or(RankQuery::Mode(Mode::Std));
    let guild = m.guild_id.expect("Guild-only command");

    let osu_users = env
        .saved_users
        .all()
        .await?
        .into_iter()
        .map(|v| (v.user_id, v))
        .collect::<HashMap<_, _>>();

    let users = env
        .prelude
        .members
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

    const ITEMS_PER_PAGE: usize = 10;
    let users = Arc::new(users);
    let last_update = last_update.unwrap();
    let total_len = users.len();
    let total_pages = (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    paginate_reply(
        paginate_from_fn(move |page: u8, ctx: &Context, m: &mut Message| {
            use Align::*;
            let users = users.clone();
            Box::pin(async move {
                let start = (page as usize) * ITEMS_PER_PAGE;
                let end = (start + ITEMS_PER_PAGE).min(users.len());
                if start >= end {
                    return Ok(false);
                }
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
                        total_pages,
                        last_update.format("<t:%s:R>"),
                    ))
                    .build();
                m.edit(ctx, EditMessage::new().content(content)).await?;
                Ok(true)
            })
        })
        .with_page_count(total_pages),
        ctx,
        m,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderBy {
    PP,
    Score,
}

impl Default for OrderBy {
    fn default() -> Self {
        Self::PP
    }
}

impl FromStr for OrderBy {
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
pub async fn show_leaderboard(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let order = args.single::<OrderBy>().unwrap_or_default();
    let style = args.single::<ScoreListStyle>().unwrap_or_default();
    let guild = msg.guild_id.expect("Guild-only command");
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let bm = match super::load_beatmap(&env, msg.channel_id, msg.referenced_message.as_ref()).await
    {
        Some((bm, _)) => bm,
        None => {
            msg.reply(&ctx, "No beatmap queried on this channel.")
                .await?;
            return Ok(());
        }
    };

    let scores = {
        let reaction = msg.react(ctx, '⌛').await?;
        let s = get_leaderboard(&ctx, &env, &bm, order, guild).await?;
        reaction.delete(&ctx).await?;
        s
    };

    if scores.is_empty() {
        msg.reply(&ctx, "No scores have been recorded for this beatmap.")
            .await?;
        return Ok(());
    }

    match style {
        ScoreListStyle::Table => {
            let reply = msg
                .reply(
                    &ctx,
                    format!(
                        "⌛ Loading top scores on beatmap `{}`...",
                        bm.short_link(Mods::NOMOD)
                    ),
                )
                .await?;
            display_rankings_table(&ctx, reply, scores, &bm, order).await?;
        }
        ScoreListStyle::Grid => {
            let reply = msg
                .reply(
                    &ctx,
                    format!(
                        "Here are the top scores on beatmap `{}` of this server!",
                        bm.short_link(Mods::NOMOD)
                    ),
                )
                .await?;
            style
                .display_scores(
                    scores.into_iter().map(|s| s.score).collect(),
                    bm.1,
                    ctx,
                    Some(guild),
                    reply,
                )
                .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Ranking {
    pub pp: f64,        // calculated pp or score pp
    pub official: bool, // official = pp is from bancho
    pub member: Arc<String>,
    pub score: Score,
}

pub async fn get_leaderboard(
    ctx: &Context,
    env: &OsuEnv,
    bm: &BeatmapWithMode,
    order: OrderBy,
    guild: GuildId,
) -> Result<Vec<Ranking>> {
    let BeatmapWithMode(beatmap, mode) = bm;
    let oppai_map = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
    let osu_users = env
        .saved_users
        .all()
        .await?
        .into_iter()
        .map(|v| (v.user_id, v))
        .collect::<HashMap<_, _>>();
    let mut scores = env
        .prelude
        .members
        .query_members(&ctx, guild)
        .await?
        .iter()
        .filter_map(|m| osu_users.get(&m.user.id).map(|ou| (m.distinct(), ou.id)))
        .map(|(mem, osu_id)| {
            env.client
                .scores(bm.0.beatmap_id, move |f| {
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
                                *mode,
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
                    Some(Ranking {
                        pp: pp.1,
                        official: pp.0,
                        member: mem.clone(),
                        score,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    match order {
        OrderBy::PP => scores.sort_by(|a, b| {
            (b.official, b.pp)
                .partial_cmp(&(a.official, a.pp))
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        OrderBy::Score => {
            scores.sort_by(|a, b| b.score.normalized_score.cmp(&a.score.normalized_score))
        }
    };
    Ok(scores)
}

pub async fn display_rankings_table(
    ctx: &Context,
    to: Message,
    scores: Vec<Ranking>,
    bm: &BeatmapWithMode,
    order: OrderBy,
) -> Result<()> {
    let has_lazer_score = scores.iter().any(|v| v.score.score.is_none());

    const ITEMS_PER_PAGE: usize = 5;
    let total_len = scores.len();
    let total_pages = (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;

    paginate_with_first_message(
        paginate_from_fn(move |page: u8, ctx: &Context, m: &mut Message| {
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return Box::pin(future::ready(Ok(false)));
            }
            let scores = scores[start..end].to_vec();
            let bm = (bm.0.clone(), bm.1);
            Box::pin(async move {
                const SCORE_HEADERS: [&'static str; 8] =
                    ["#", "Score", "Mods", "Rank", "Acc", "Combo", "Miss", "User"];
                const PP_HEADERS: [&'static str; 8] =
                    ["#", "PP", "Mods", "Rank", "Acc", "Combo", "Miss", "User"];
                const ALIGNS: [Align; 8] = [Right, Right, Right, Right, Right, Right, Right, Left];

                let score_arr = scores
                    .iter()
                    .enumerate()
                    .map(
                        |(
                            id,
                            Ranking {
                                pp,
                                official,
                                member,
                                score,
                            },
                        )| {
                            [
                                format!("{}", 1 + id + start),
                                match order {
                                    OrderBy::PP => {
                                        format!("{:.2}{}", pp, if *official { "" } else { "[?]" })
                                    }
                                    OrderBy::Score => {
                                        crate::discord::embeds::grouped_number(if has_lazer_score {
                                            score.normalized_score as u64
                                        } else {
                                            score.score.unwrap()
                                        })
                                    }
                                },
                                score.mods.to_string(),
                                score.rank.to_string(),
                                format!("{:.2}%", score.accuracy(bm.1)),
                                format!("{}x", score.max_combo),
                                format!("{}", score.count_miss),
                                member.to_string(),
                            ]
                        },
                    )
                    .collect::<Vec<_>>();

                let score_table = match order {
                    OrderBy::PP => table_formatting(&PP_HEADERS, &ALIGNS, score_arr),
                    OrderBy::Score => table_formatting(&SCORE_HEADERS, &ALIGNS, score_arr),
                };
                let content = MessageBuilder::new()
                    .push_line(score_table)
                    .push_line(format!(
                        "Page **{}**/**{}**. Not seeing your scores? Run `osu check` to update.",
                        page + 1,
                        total_pages,
                    ))
                    .push(
                        if let crate::models::ApprovalStatus::Ranked(_) = bm.0.approval {
                            ""
                        } else if order == OrderBy::PP {
                            "PP was calculated by `oppai-rs`, **not** official values.\n"
                        } else {
                            ""
                        },
                    )
                    .build();

                m.edit(&ctx, EditMessage::new().content(content)).await?;
                Ok(true)
            })
        })
        .with_page_count(total_pages),
        ctx,
        to,
        std::time::Duration::from_secs(60),
    )
    .await?;
    Ok(())
}

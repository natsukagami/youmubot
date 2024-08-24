use std::{borrow::Cow, cmp::Ordering, collections::HashMap, str::FromStr, sync::Arc};

use chrono::DateTime;
use pagination::paginate_with_first_message;
use serenity::{
    all::{GuildId, Member},
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
    discord::{db::OsuUser, display::ScoreListStyle, oppai_cache::Accuracy, BeatmapWithMode},
    models::{Mode, Mods},
    request::UserID,
    Score,
};

use super::{ModeArg, OsuEnv};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RankQuery {
    #[default]
    PP,
    TotalPP,
    MapLength,
    MapAge {
        newest_first: bool,
    },
}

impl RankQuery {
    fn col_name(&self) -> &'static str {
        match self {
            RankQuery::PP => "pp",
            RankQuery::TotalPP => "Total pp",
            RankQuery::MapLength => "Map length",
            RankQuery::MapAge { newest_first: _ } => "Map age",
        }
    }
    fn pass_pp_limit(&self, mode: Mode, ou: &OsuUser) -> bool {
        match self {
            RankQuery::PP | RankQuery::TotalPP => true,
            RankQuery::MapAge { newest_first: _ } | RankQuery::MapLength => {
                ou.modes.get(&mode).is_some_and(|v| v.pp >= 500.0)
            }
        }
    }
    fn extract_row(&self, mode: Mode, ou: &OsuUser) -> Cow<'static, str> {
        match self {
            RankQuery::PP => ou
                .modes
                .get(&mode)
                .map(|v| format!("{:.02}", v.pp).into())
                .unwrap_or_else(|| "-".into()),
            RankQuery::TotalPP => {
                format!("{:.02}", ou.modes.values().map(|v| v.pp).sum::<f64>()).into()
            }
            RankQuery::MapLength => ou
                .modes
                .get(&mode)
                .map(|v| {
                    let len = v.map_length;
                    let trunc_secs = len.floor() as u64;
                    let minutes = trunc_secs / 60;
                    let seconds = len - (60 * minutes) as f64;
                    format!("{}m{:05.2}s", minutes, seconds).into()
                })
                .unwrap_or_else(|| "-".into()),
            RankQuery::MapAge { newest_first: _ } => ou
                .modes
                .get(&mode)
                .and_then(|v| DateTime::from_timestamp(v.map_age, 0))
                .map(|time| time.format("%F %T").to_string().into())
                .unwrap_or_else(|| "-".into()),
        }
    }
}

impl FromStr for RankQuery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pp" => Ok(RankQuery::PP),
            "total" | "total-pp" => Ok(RankQuery::TotalPP),
            "map-length" => Ok(RankQuery::MapLength),
            "age" | "map-age" => Ok(RankQuery::MapAge { newest_first: true }),
            "old" | "age-old" | "map-age-old" => Ok(RankQuery::MapAge {
                newest_first: false,
            }),
            _ => Err(format!("not a query: {}", s)),
        }
    }
}

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(2)]
#[only_in(guilds)]
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let mode = args.find::<ModeArg>().map(|v| v.0).unwrap_or(Mode::Std);
    let query = args.find::<RankQuery>().unwrap_or_default();
    let guild = m.guild_id.expect("Guild-only command");

    let mut users = env
        .saved_users
        .all()
        .await?
        .into_iter()
        .filter(|v| query.pass_pp_limit(mode, v))
        .map(|v| (v.user_id, v))
        .collect::<HashMap<_, _>>();
    let mut users = env
        .prelude
        .members
        .query_members(&ctx, guild)
        .await?
        .iter()
        .filter_map(|m| users.remove(&m.user.id).map(|ou| (m.clone(), ou)))
        .collect::<Vec<_>>();
    let last_update = users
        .iter()
        .filter_map(|(_, u)| {
            if query == RankQuery::TotalPP {
                u.modes.values().map(|v| v.last_update).min()
            } else {
                u.modes.get(&mode).map(|v| v.last_update)
            }
        })
        .min();
    type Item = (Member, OsuUser);
    #[allow(clippy::type_complexity)]
    let sort_fn: Box<dyn Fn(&Item, &Item) -> Ordering> = match query {
        RankQuery::PP => Box::new(|(_, a), (_, b)| {
            a.modes
                .get(&mode)
                .map(|v| v.pp)
                .partial_cmp(&b.modes.get(&mode).map(|v| v.pp))
                .unwrap()
                .reverse()
        }),
        RankQuery::TotalPP => Box::new(|(_, a), (_, b)| {
            a.modes
                .values()
                .map(|v| v.pp)
                .sum::<f64>()
                .partial_cmp(&b.modes.values().map(|v| v.pp).sum())
                .unwrap()
                .reverse()
        }),
        RankQuery::MapLength => Box::new(|(_, a), (_, b)| {
            a.modes
                .get(&mode)
                .map(|v| v.map_length)
                .partial_cmp(&b.modes.get(&mode).map(|v| v.map_length))
                .unwrap()
                .reverse()
        }),
        RankQuery::MapAge { newest_first } => Box::new(move |(_, a), (_, b)| {
            let r = a
                .modes
                .get(&mode)
                .map(|v| v.map_age)
                .partial_cmp(&b.modes.get(&mode).map(|v| v.map_age))
                .unwrap();
            if newest_first {
                r.reverse()
            } else {
                r
            }
        }),
    };
    users.sort_unstable_by(sort_fn);

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
                let table = match query {
                    RankQuery::MapAge { newest_first: _ } | RankQuery::MapLength => {
                        let headers = ["#", query.col_name(), "pp", "Username", "Member"];
                        const ALIGNS: [Align; 5] = [Right, Right, Right, Left, Left];

                        let table = users
                            .iter()
                            .enumerate()
                            .map(|(i, (mem, ou))| {
                                [
                                    format!("{}", 1 + i + start),
                                    query.extract_row(mode, ou).to_string(),
                                    RankQuery::PP.extract_row(mode, ou).to_string(),
                                    ou.username.to_string(),
                                    mem.distinct(),
                                ]
                            })
                            .collect::<Vec<_>>();
                        table_formatting(&headers, &ALIGNS, table)
                    }
                    RankQuery::PP => {
                        const HEADERS: [&str; 6] =
                            ["#", "pp", "Map length", "Map age", "Username", "Member"];
                        const ALIGNS: [Align; 6] = [Right, Right, Right, Right, Left, Left];

                        let table = users
                            .iter()
                            .enumerate()
                            .map(|(i, (mem, ou))| {
                                [
                                    format!("{}", 1 + i + start),
                                    RankQuery::PP.extract_row(mode, ou).to_string(),
                                    RankQuery::MapLength.extract_row(mode, ou).to_string(),
                                    (RankQuery::MapAge {
                                        newest_first: false,
                                    })
                                    .extract_row(mode, ou)
                                    .to_string(),
                                    ou.username.to_string(),
                                    mem.distinct(),
                                ]
                            })
                            .collect::<Vec<_>>();
                        table_formatting(&HEADERS, &ALIGNS, table)
                    }
                    RankQuery::TotalPP => {
                        const HEADERS: [&str; 4] = ["#", "Total pp", "Username", "Member"];
                        const ALIGNS: [Align; 4] = [Right, Right, Left, Left];

                        let table = users
                            .iter()
                            .enumerate()
                            .map(|(i, (mem, ou))| {
                                [
                                    format!("{}", 1 + i + start),
                                    query.extract_row(mode, ou).to_string(),
                                    ou.username.to_string(),
                                    mem.distinct(),
                                ]
                            })
                            .collect::<Vec<_>>();
                        table_formatting(&HEADERS, &ALIGNS, table)
                    }
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
        let s = get_leaderboard(ctx, &env, &bm, order, guild).await?;
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
            display_rankings_table(ctx, reply, scores, &bm, order).await?;
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
                                &score.mods,
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
                .unwrap_or(Ordering::Equal)
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
                const SCORE_HEADERS: [&str; 8] =
                    ["#", "Score", "Mods", "Rank", "Acc", "Combo", "Miss", "User"];
                const PP_HEADERS: [&str; 8] =
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

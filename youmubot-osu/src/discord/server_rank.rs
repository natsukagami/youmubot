use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    future::Future,
    str::FromStr,
    sync::Arc,
};

use chrono::DateTime;
use pagination::paginate_with_first_message;
use serenity::{
    all::{GuildId, Member, PartialGuild},
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
    discord::{
        db::OsuUser, display::ScoreListStyle, link_parser::EmbedType, oppai_cache::Stats,
        time_before_now,
    },
    models::Mode,
    request::UserID,
    Beatmap, Score,
};

use super::{ModeArg, OsuEnv};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, poise::ChoiceParameter)]
pub(crate) enum RankQuery {
    #[default]
    PP,
    #[name = "Total PP"]
    TotalPP,
    #[name = "Weighted Map Length"]
    MapLength,
    #[name = "Map Age"]
    MapAge,
}

impl RankQuery {
    fn col_name(&self) -> &'static str {
        match self {
            RankQuery::PP => "pp",
            RankQuery::TotalPP => "Total pp",
            RankQuery::MapLength => "Map length",
            RankQuery::MapAge => "Map age",
        }
    }
    fn pass_pp_limit(&self, mode: Mode, ou: &OsuUser) -> bool {
        match self {
            RankQuery::PP | RankQuery::TotalPP => true,
            RankQuery::MapAge | RankQuery::MapLength => {
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
            RankQuery::MapAge => ou
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
            "age" | "map-age" => Ok(RankQuery::MapAge),
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
    let mode = args.find::<ModeArg>().map(|v| v.0).ok();
    let query = args.find::<RankQuery>().ok();
    let guild = m
        .guild_id
        .expect("Guild-only command")
        .to_partial_guild(&ctx)
        .await?;

    let ctxc = ctx.clone();
    do_server_ranks(ctx, &env, &guild, mode, query, false, |msg| async {
        let m = m.reply(&ctxc, msg).await?;
        Ok(m) as Result<_>
    })
    .await?;

    Ok(())
}

pub(crate) async fn do_server_ranks<T>(
    ctx: &Context,
    env: &OsuEnv,
    guild: &PartialGuild,
    mode: Option<Mode>,
    query: Option<RankQuery>,
    reverse: bool,
    mk_initial_message: impl FnOnce(String) -> T,
) -> Result<()>
where
    T: Future<Output = Result<Message>>,
{
    let mode = mode.unwrap_or(Mode::Std);
    let query = query.unwrap_or(RankQuery::PP);
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
        .query_members(&ctx, guild.id)
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
        RankQuery::MapAge => Box::new(move |(_, a), (_, b)| {
            a.modes
                .get(&mode)
                .map(|v| v.map_age)
                .partial_cmp(&b.modes.get(&mode).map(|v| v.map_age))
                .unwrap()
        }),
    };
    users.sort_unstable_by(sort_fn);
    if reverse {
        users.reverse();
    }

    if users.is_empty() {
        mk_initial_message("No saved users in the current server...".to_owned()).await?;
        return Ok(());
    }

    let header = format!(
        "Rankings for **{}**, ordered by _{}{}_",
        guild.name,
        query.col_name(),
        if reverse { " (reversed)" } else { "" },
    );

    let msg = mk_initial_message(header.clone()).await?;

    const ITEMS_PER_PAGE: usize = 10;
    let users = Arc::new(users);
    let last_update = last_update.unwrap();
    let total_len = users.len();
    let total_pages = (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    paginate_with_first_message(
        paginate_from_fn(move |page: u8, _: &Context, _: &Message, btns| {
            let header = header.clone();
            use Align::*;
            let users = users.clone();
            Box::pin(async move {
                let start = (page as usize) * ITEMS_PER_PAGE;
                let end = (start + ITEMS_PER_PAGE).min(users.len());
                if start >= end {
                    return Ok(None);
                }
                let users = &users[start..end];
                let table = match query {
                    RankQuery::MapAge | RankQuery::MapLength => {
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
                                    RankQuery::MapAge.extract_row(mode, ou).to_string(),
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
                    .push_line(header)
                    .push_line(table)
                    .push_line(format!(
                        "Page **{}**/**{}**. Last updated: {}",
                        page + 1,
                        total_pages,
                        last_update.format("<t:%s:R>"),
                    ))
                    .build();
                Ok(Some(
                    EditMessage::new().content(content).components(vec![btns]),
                ))
            })
        })
        .with_page_count(total_pages),
        ctx,
        msg,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter)]
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

struct AllLb;
impl FromStr for AllLb {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--all" => Ok(AllLb),
            _ => Err(Error::msg("unknown value")),
        }
    }
}

#[command("leaderboard")]
#[aliases("lb", "bmranks", "br", "cc", "updatelb")]
#[usage = "[--all to show all scores, not just ranked] / [--score to sort by score, default to sort by pp] / [--table to show a table, --grid to show score by score] / [mods to filter]"]
#[description = "See the server's ranks on the last seen beatmap"]
#[max_args(2)]
#[only_in(guilds)]
pub async fn show_leaderboard(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let show_all = args.single::<AllLb>().is_ok();
    let order = args.single::<OrderBy>().unwrap_or_default();
    let style = args.single::<ScoreListStyle>().unwrap_or_default();
    let guild = msg.guild_id.expect("Guild-only command");
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let Some(beatmap) = super::load_beatmap(
        &env,
        msg.channel_id,
        msg.referenced_message.as_ref(),
        crate::discord::LoadRequest::Any,
    )
    .await
    else {
        msg.reply(&ctx, "No beatmap queried on this channel.")
            .await?;
        return Ok(());
    };
    let scoreboard_msg = beatmap.mention();
    let (scores, show_diff) =
        get_leaderboard_from_embed(ctx, &env, beatmap, None, show_all, order, guild).await?;

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
                    format!("⌛ Loading top scores on {}...", scoreboard_msg),
                )
                .await?;
            display_rankings_table(ctx, reply, scores, show_diff, order).await?;
        }
        ScoreListStyle::Grid => {
            let reply = msg
                .reply(
                    &ctx,
                    format!(
                        "Here are the top scores on {} of this server!",
                        scoreboard_msg
                    ),
                )
                .await?;
            style
                .display_scores(
                    scores.into_iter().map(|s| s.score).collect(),
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
    pub beatmap: Arc<Beatmap>,
    pub score: Score,
    pub star: f64,
}

async fn get_leaderboard(
    ctx: &Context,
    env: &OsuEnv,
    beatmaps: impl IntoIterator<Item = Beatmap>,
    mode_override: Option<Mode>,
    show_unranked: bool,
    order: OrderBy,
    guild: GuildId,
) -> Result<Vec<Ranking>> {
    let oppai_maps = beatmaps
        .into_iter()
        .map(|b| async move {
            let op = env.oppai.get_beatmap(b.beatmap_id).await?;
            let r: Result<_> = Ok((Arc::new(b), op));
            r
        })
        .collect::<stream::FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?;
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
        .filter_map(|m| {
            osu_users
                .get(&m.user.id)
                .map(|ou| (Arc::new(m.distinct()), ou.id))
        })
        .flat_map(|(mem, osu_id)| {
            oppai_maps.iter().map(move |(b, op)| {
                let mem = mem.clone();
                env.client
                    .scores(b.beatmap_id, move |f| {
                        f.user(UserID::ID(osu_id)).mode(mode_override)
                    })
                    .map(move |r| Some((b, op, mem.clone(), r.ok()?)))
            })
        })
        .collect::<FuturesUnordered<_>>()
        .filter_map(future::ready)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flat_map(|(b, op, mem, scores)| {
            scores
                .into_iter()
                .map(|score| {
                    let pp = score.pp.map(|v| (true, v)).unwrap_or_else(|| {
                        (
                            false,
                            op.get_pp_from(
                                mode_override.unwrap_or(b.mode),
                                Some(score.max_combo),
                                Stats::Raw(&score.statistics),
                                &score.mods,
                            ),
                        )
                    });
                    let info = op.get_info_with(score.mode, &score.mods);
                    Ranking {
                        pp: pp.1,
                        official: pp.0,
                        beatmap: b.clone(),
                        member: mem.clone(),
                        star: info.attrs.stars(),
                        score,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    if !show_unranked {
        let mut mp = BTreeMap::<u64 /* user id */, Vec<Ranking>>::new();
        for r in scores.drain(0..scores.len()) {
            let rs = mp.entry(r.score.user_id).or_default();
            match rs.iter_mut().find(|t| t.score.mods == r.score.mods) {
                Some(t) => {
                    if t.pp < r.pp {
                        *t = r;
                    }
                }
                None => {
                    rs.push(r);
                }
            }
        }
        scores = mp.into_values().flatten().collect();
    }

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

pub async fn get_leaderboard_from_embed(
    ctx: &Context,
    env: &OsuEnv,
    embed: EmbedType,
    mode_override: Option<Mode>,
    show_unranked: bool,
    order: OrderBy,
    guild: GuildId,
) -> Result<(Vec<Ranking>, bool /* should show diff */)> {
    Ok(match embed {
        EmbedType::Beatmap(map, mode, _, _) => {
            let iter = std::iter::once(*map);
            let scores = get_leaderboard(
                ctx,
                &env,
                iter,
                mode_override.or(mode),
                show_unranked,
                order,
                guild,
            )
            .await?;
            (scores, false)
        }
        EmbedType::Beatmapset(maps, _) if maps.is_empty() => (vec![], false),
        EmbedType::Beatmapset(maps, mode) => {
            let show_diff = maps.len() > 1;
            (
                get_leaderboard(
                    ctx,
                    &env,
                    maps,
                    mode_override.or(mode),
                    show_unranked,
                    order,
                    guild,
                )
                .await?,
                show_diff,
            )
        }
    })
}

pub async fn display_rankings_table(
    ctx: &Context,
    to: Message,
    scores: Vec<Ranking>,
    show_diff: bool,
    order: OrderBy,
) -> Result<()> {
    let has_lazer_score = scores.iter().any(|v| v.score.mods.is_lazer);

    const ITEMS_PER_PAGE: usize = 5;
    let total_len = scores.len();
    let total_pages = (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let header = Arc::new(to.content.clone());

    paginate_with_first_message(
        paginate_from_fn(move |page: u8, _, _, btns| {
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return Box::pin(future::ready(Ok(None)));
            }
            let scores = scores[start..end].to_vec();
            let header = header.clone();
            Box::pin(async move {
                let headers: [&'static str; 9] = [
                    "#",
                    match order {
                        OrderBy::PP => "pp",
                        OrderBy::Score => "Score",
                    },
                    if show_diff { "Map" } else { "Mods" },
                    "Rank",
                    "Acc",
                    "Combo",
                    "Miss",
                    "When",
                    "User",
                ];
                let aligns: [Align; 9] = [
                    Right,
                    Right,
                    if show_diff { Left } else { Right },
                    Right,
                    Right,
                    Right,
                    Right,
                    Right,
                    Left,
                ];

                let score_arr = scores
                    .iter()
                    .enumerate()
                    .map(
                        |(
                            id,
                            Ranking {
                                pp,
                                beatmap,
                                official,
                                member,
                                score,
                                star,
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
                                            score.score
                                        })
                                    }
                                },
                                if show_diff {
                                    let trimmed_diff = if beatmap.difficulty_name.len() > 20 {
                                        let mut s = beatmap.difficulty_name.clone();
                                        s.truncate(17);
                                        s + "..."
                                    } else {
                                        beatmap.difficulty_name.clone()
                                    };
                                    format!(
                                        "[{:.2}*] {} {}",
                                        star,
                                        trimmed_diff,
                                        score.mods.to_string()
                                    )
                                } else {
                                    score.mods.to_string()
                                },
                                score.rank.to_string(),
                                format!("{:.2}%", score.accuracy(score.mode)),
                                format!("{}x", score.max_combo),
                                format!("{}", score.count_miss),
                                time_before_now(&score.date),
                                member.to_string(),
                            ]
                        },
                    )
                    .collect::<Vec<_>>();

                let score_table = table_formatting(&headers, &aligns, score_arr);
                let content = MessageBuilder::new()
                    .push_line(header.as_ref())
                    .push_line(score_table)
                    .push_line(format!(
                        "Page **{}**/**{}**. Not seeing your scores? Run `osu check` to update.",
                        page + 1,
                        total_pages,
                    ))
                    .build();

                Ok(Some(
                    EditMessage::new().content(content).components(vec![btns]),
                ))
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

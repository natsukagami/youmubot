use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::oppai_cache::BeatmapCache,
    models::{Beatmap, Mode, Mods, Score, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuHttpClient,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
};
use std::{str::FromStr, sync::Arc};
use youmubot_prelude::*;

mod announcer;
pub(crate) mod beatmap_cache;
mod cache;
mod db;
pub(crate) mod embeds;
mod hook;
pub(crate) mod oppai_cache;
mod server_rank;

use db::OsuUser;
use db::{OsuLastBeatmap, OsuSavedUsers};
use embeds::{beatmap_embed, score_embed, user_embed};
pub use hook::hook;
use server_rank::SERVER_RANK_COMMAND;

/// The osu! client.
pub(crate) struct OsuClient;

impl TypeMapKey for OsuClient {
    type Value = Arc<OsuHttpClient>;
}

/// Sets up the osu! command handling section.
///
/// This automatically enables:
///  - Related databases
///  - An announcer system (that will eventually be revamped)
///  - The osu! API client.
///
///  This does NOT automatically enable:
///  - Commands on the "osu" prefix
///  - Hooks. Hooks are completely opt-in.
///  
pub async fn setup(
    path: &std::path::Path,
    data: &mut TypeMap,
    announcers: &mut AnnouncerHandler,
) -> CommandResult {
    // Databases
    OsuSavedUsers::insert_into(&mut *data, &path.join("osu_saved_users.yaml"))?;
    OsuLastBeatmap::insert_into(&mut *data, &path.join("last_beatmaps.yaml"))?;

    // API client
    let http_client = data.get::<HTTPClient>().unwrap().clone();
    let osu_client = Arc::new(OsuHttpClient::new(
        std::env::var("OSU_API_KEY").expect("Please set OSU_API_KEY as osu! api key."),
    ));
    data.insert::<OsuClient>(osu_client.clone());
    data.insert::<oppai_cache::BeatmapCache>(oppai_cache::BeatmapCache::new(http_client));
    data.insert::<beatmap_cache::BeatmapMetaCache>(beatmap_cache::BeatmapMetaCache::new(
        osu_client,
    ));

    // Announcer
    announcers.add(announcer::ANNOUNCER_KEY, announcer::Announcer);
    Ok(())
}

#[group]
#[prefix = "osu"]
#[description = "osu! related commands."]
#[commands(std, taiko, catch, mania, save, recent, last, check, top, server_rank)]
#[default_command(std)]
struct Osu;

#[command]
#[aliases("osu", "osu!")]
#[description = "Receive information about an user in osu!std mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn std(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Std).await
}

#[command]
#[aliases("osu!taiko")]
#[description = "Receive information about an user in osu!taiko mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn taiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Taiko).await
}

#[command]
#[aliases("fruits", "osu!catch", "ctb")]
#[description = "Receive information about an user in osu!catch mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn catch(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Catch).await
}

#[command]
#[aliases("osu!mania")]
#[description = "Receive information about an user in osu!mania mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn mania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Mania).await
}

pub(crate) struct BeatmapWithMode(pub Beatmap, pub Mode);

impl BeatmapWithMode {
    fn mode(&self) -> Mode {
        self.1
    }
}

impl AsRef<Beatmap> for BeatmapWithMode {
    fn as_ref(&self) -> &Beatmap {
        &self.0
    }
}

#[command]
#[description = "Save the given username as your username."]
#[usage = "[username or user_id]"]
#[num_args(1)]
pub async fn save(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let osu = data.get::<OsuClient>().unwrap();

    let user = args.single::<String>()?;
    let user: Option<User> = osu.user(UserID::Auto(user), |f| f).await?;
    match user {
        Some(u) => {
            OsuSavedUsers::open(&*data).borrow_mut()?.insert(
                msg.author.id,
                OsuUser {
                    id: u.id,
                    last_update: chrono::Utc::now(),
                    pp: vec![],
                },
            );
            msg.reply(
                &ctx,
                MessageBuilder::new()
                    .push("user has been set to ")
                    .push_mono_safe(u.username)
                    .build(),
            )
            .await?;
        }
        None => {
            msg.reply(&ctx, "user not found...").await?;
        }
    }
    Ok(())
}

struct ModeArg(Mode);

impl FromStr for ModeArg {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ModeArg(match &s.to_lowercase()[..] {
            "osu" | "std" => Mode::Std,
            "taiko" | "osu!taiko" => Mode::Taiko,
            "ctb" | "fruits" | "catch" | "osu!ctb" | "osu!catch" => Mode::Catch,
            "osu!mania" | "mania" => Mode::Mania,
            _ => return Err(format!("Unknown mode {}", s)),
        }))
    }
}

fn to_user_id_query(
    s: Option<UsernameArg>,
    data: &TypeMap,
    msg: &Message,
) -> Result<UserID, Error> {
    let id = match s {
        Some(UsernameArg::Raw(s)) => return Ok(UserID::Auto(s)),
        Some(UsernameArg::Tagged(r)) => r,
        None => msg.author.id,
    };

    let db = OsuSavedUsers::open(data);
    let db = db.borrow()?;
    db.get(&id)
        .cloned()
        .map(|u| UserID::ID(u.id))
        .ok_or(Error::from("No saved account found"))
}

enum Nth {
    All,
    Nth(u8),
}

impl FromStr for Nth {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "--all" || s == "-a" || s == "##" {
            Ok(Nth::All)
        } else if !s.starts_with("#") {
            Err(Error::from("Not an order"))
        } else {
            let v = s.split_at("#".len()).1.parse()?;
            Ok(Nth::Nth(v))
        }
    }
}

async fn list_plays<'a>(
    plays: Vec<Score>,
    mode: Mode,
    ctx: &'a Context,
    m: &'a Message,
) -> CommandResult {
    let plays = Arc::new(plays);
    if plays.is_empty() {
        m.reply(&ctx, "No plays found").await?;
        return Ok(());
    }

    const ITEMS_PER_PAGE: usize = 5;
    let total_pages = (plays.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    paginate(
        move |page, ctx, msg| {
            let plays = plays.clone();
            Box::pin(async move {
                let data = ctx.data.read().await;
                let osu = data.get::<BeatmapMetaCache>().unwrap();
                let beatmap_cache = data.get::<BeatmapCache>().unwrap();
                let page = page as usize;
                let start = page * ITEMS_PER_PAGE;
                let end = plays.len().min(start + ITEMS_PER_PAGE);
                if start >= end {
                    return Ok(false);
                }

                let plays = &plays[start..end];
                let beatmaps = plays
                    .iter()
                    .map(|play| async move {
                        let beatmap = osu.get_beatmap(play.beatmap_id, mode).await?;
                        let stars = {
                            let b = beatmap_cache.get_beatmap(beatmap.beatmap_id).await?;
                            mode.to_oppai_mode()
                                .and_then(|mode| b.get_info_with(Some(mode), play.mods).ok())
                                .map(|info| info.stars as f64)
                                .unwrap_or(beatmap.difficulty.stars)
                        };
                        let r: Result<_> = Ok(format!(
                            "[{:.1}*] {} - {} [{}] ({})",
                            stars,
                            beatmap.artist,
                            beatmap.title,
                            beatmap.difficulty_name,
                            beatmap.short_link(Some(mode), Some(play.mods)),
                        ));
                        r
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.unwrap_or("FETCH_FAILED".to_owned()))
                    .collect::<Vec<String>>();
                let pp = plays
                    .iter()
                    .map(|p| async move {
                        match p.pp.map(|pp| format!("{:.2}pp", pp)) {
                            Some(v) => Ok(v),
                            None => {
                                let b = beatmap_cache.get_beatmap(p.beatmap_id).await?;
                                let r: Result<_> = Ok(mode
                                    .to_oppai_mode()
                                    .and_then(|op| {
                                        b.get_pp_from(
                                            oppai_rs::Combo::NonFC {
                                                max_combo: p.max_combo as u32,
                                                misses: p.count_miss as u32,
                                            },
                                            p.accuracy(mode) as f32,
                                            Some(op),
                                            p.mods,
                                        )
                                        .ok()
                                        .map(|pp| format!("{:.2}pp [?]", pp))
                                    })
                                    .unwrap_or("-".to_owned()));
                                r
                            }
                        }
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.unwrap_or("-".to_owned()))
                    .collect::<Vec<String>>();
                let (beatmaps, pp) = future::join(beatmaps, pp).await;
                let pw = pp.iter().map(|v| v.len()).max().unwrap_or(2);
                /*mods width*/
                let mw = plays
                    .iter()
                    .map(|v| v.mods.to_string().len())
                    .max()
                    .unwrap()
                    .max(4);
                /*beatmap names*/
                let bw = beatmaps.iter().map(|v| v.len()).max().unwrap().max(7);

                let mut m = MessageBuilder::new();
                // Table header
                m.push_line(format!(
                    " #  | {:pw$} | accuracy | rank | {:mw$} | {:bw$}",
                    "pp",
                    "mods",
                    "beatmap",
                    pw = pw,
                    mw = mw,
                    bw = bw
                ));
                m.push_line(format!(
                    "------{:-<pw$}---------------------{:-<mw$}---{:-<bw$}",
                    "",
                    "",
                    "",
                    pw = pw,
                    mw = mw,
                    bw = bw
                ));
                // Each row
                for (id, (play, beatmap)) in plays.iter().zip(beatmaps.iter()).enumerate() {
                    m.push_line(format!(
                        "{:>3} | {:>pw$} | {:>8} | {:^4} | {:mw$} | {:bw$}",
                        id + start + 1,
                        pp[id],
                        format!("{:.2}%", play.accuracy(mode)),
                        play.rank.to_string(),
                        play.mods.to_string(),
                        beatmap,
                        pw = pw,
                        mw = mw,
                        bw = bw
                    ));
                }
                // End
                let table = m.build().replace("```", "\\`\\`\\`");
                let mut m = MessageBuilder::new();
                m.push_codeblock(table, None).push_line(format!(
                    "Page **{}/{}**",
                    page + 1,
                    total_pages
                ));
                if let None = mode.to_oppai_mode() {
                    m.push_line("Note: star difficulty doesn't reflect mods applied.");
                } else {
                    m.push_line("[?] means pp was predicted by oppai-rs.");
                }
                msg.edit(ctx, |f| f.content(m.to_string())).await?;
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

#[command]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = --all] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[max_args(3)]
pub async fn recent(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let mode = args.single::<ModeArg>().unwrap_or(ModeArg(Mode::Std)).0;
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*data, msg)?;

    let osu = data.get::<OsuClient>().unwrap();
    let meta_cache = data.get::<BeatmapMetaCache>().unwrap();
    let oppai = data.get::<BeatmapCache>().unwrap();
    let user = osu
        .user(user, |f| f.mode(mode))
        .await?
        .ok_or(Error::from("User not found"))?;
    match nth {
        Nth::Nth(nth) => {
            let recent_play = osu
                .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(nth))
                .await?
                .into_iter()
                .last()
                .ok_or(Error::from("No such play"))?;
            let beatmap = meta_cache.get_beatmap(recent_play.beatmap_id, mode).await?;
            let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(&ctx, |m| {
                    m.content(format!(
                        "{}: here is the play that you requested",
                        msg.author
                    ))
                    .embed(|m| score_embed(&recent_play, &beatmap_mode, &content, &user, None, m))
                })
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&*data, msg.channel_id, &beatmap_mode)?;
        }
        Nth::All => {
            let plays = osu
                .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(50))
                .await?;
            list_plays(plays, mode, ctx, msg).await?;
        }
    }
    Ok(())
}

#[command]
#[description = "Show information from the last queried beatmap."]
#[usage = "[mods = no mod]"]
#[max_args(1)]
pub async fn last(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let b = cache::get_beatmap(&*data, msg.channel_id)?;

    match b {
        Some(BeatmapWithMode(b, m)) => {
            let mods = args.find::<Mods>().unwrap_or(Mods::NOMOD);
            let info = data
                .get::<BeatmapCache>()
                .unwrap()
                .get_beatmap(b.beatmap_id)
                .await?
                .get_info_with(m.to_oppai_mode(), mods)
                .ok();
            msg.channel_id
                .send_message(&ctx, |f| {
                    f.content(format!(
                        "{}: here is the beatmap you requested!",
                        msg.author
                    ))
                    .embed(|c| beatmap_embed(&b, m, mods, info, c))
                })
                .await?;
        }
        None => {
            msg.reply(&ctx, "No beatmap was queried on this channel.")
                .await?;
        }
    }

    Ok(())
}

#[command]
#[aliases("c", "chk")]
#[description = "Check your own or someone else's best record on the last beatmap."]
#[max_args(1)]
pub async fn check(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let bm = cache::get_beatmap(&*data, msg.channel_id)?;

    match bm {
        None => {
            msg.reply(&ctx, "No beatmap queried on this channel.")
                .await?;
        }
        Some(bm) => {
            let b = &bm.0;
            let m = bm.1;
            let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*data, msg)?;

            let osu = data.get::<OsuClient>().unwrap();
            let oppai = data.get::<BeatmapCache>().unwrap();

            let content = oppai.get_beatmap(b.beatmap_id).await?;

            let user = osu
                .user(user, |f| f)
                .await?
                .ok_or(Error::from("User not found"))?;
            let scores = osu
                .scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(m))
                .await?;

            if scores.is_empty() {
                msg.reply(&ctx, "No scores found").await?;
            }

            for score in scores.into_iter() {
                msg.channel_id
                    .send_message(&ctx, |c| {
                        c.embed(|m| score_embed(&score, &bm, &content, &user, None, m))
                    })
                    .await?;
            }
        }
    }

    Ok(())
}

#[command]
#[description = "Get the n-th top record of an user."]
#[usage = "[mode (std, taiko, catch, mania)] = std / #[n-th = --all] / [username or user_id = your saved user id]"]
#[example = "taiko / #2 / natsukagami"]
#[max_args(3)]
pub async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let mode = args
        .single::<ModeArg>()
        .map(|ModeArg(t)| t)
        .unwrap_or(Mode::Std);

    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*data, msg)?;

    let osu = data.get::<OsuClient>().unwrap();
    let oppai = data.get::<BeatmapCache>().unwrap();
    let user = osu
        .user(user, |f| f.mode(mode))
        .await?
        .ok_or(Error::from("User not found"))?;

    match nth {
        Nth::Nth(nth) => {
            let top_play = osu
                .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(nth))
                .await?;

            let rank = top_play.len() as u8;

            let top_play = top_play
                .into_iter()
                .last()
                .ok_or(Error::from("No such play"))?;
            let beatmap = osu
                .beatmaps(BeatmapRequestKind::Beatmap(top_play.beatmap_id), |f| {
                    f.mode(mode, true)
                })
                .await?
                .into_iter()
                .next()
                .unwrap();
            let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(&ctx, |m| {
                    m.content(format!(
                        "{}: here is the play that you requested",
                        msg.author
                    ))
                    .embed(|m| score_embed(&top_play, &beatmap, &content, &user, Some(rank), m))
                })
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&*data, msg.channel_id, &beatmap)?;
        }
        Nth::All => {
            let plays = osu
                .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))
                .await?;
            list_plays(plays, mode, ctx, msg).await?;
        }
    }
    Ok(())
}

async fn get_user(ctx: &Context, msg: &Message, mut args: Args, mode: Mode) -> CommandResult {
    let data = ctx.data.read().await;
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*data, msg)?;
    let osu = data.get::<OsuClient>().unwrap();
    let cache = data.get::<BeatmapMetaCache>().unwrap();
    let user = osu.user(user, |f| f.mode(mode)).await?;
    let oppai = data.get::<BeatmapCache>().unwrap();
    match user {
        Some(u) => {
            let best = match osu
                .user_best(UserID::ID(u.id), |f| f.limit(1).mode(mode))
                .await?
                .into_iter()
                .next()
            {
                Some(m) => {
                    let beatmap = cache.get_beatmap(m.beatmap_id, mode).await?;
                    let info = match mode.to_oppai_mode() {
                        Some(mode) => Some(
                            oppai
                                .get_beatmap(m.beatmap_id)
                                .await?
                                .get_info_with(Some(mode), m.mods)?,
                        ),
                        None => None,
                    };
                    Some((m, BeatmapWithMode(beatmap, mode), info))
                }
                None => None,
            };
            msg.channel_id
                .send_message(&ctx, |m| {
                    m.content(format!(
                        "{}: here is the user that you requested",
                        msg.author
                    ))
                    .embed(|m| user_embed(u, best, m))
                })
                .await?;
        }
        None => {
            msg.reply(&ctx, "🔍 user not found!").await?;
        }
    };
    Ok(())
}

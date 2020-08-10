use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::oppai_cache::BeatmapCache,
    models::{Beatmap, Mode, Mods, Score, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuHttpClient,
};
use rayon::prelude::*;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
};
use std::str::FromStr;
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
    type Value = OsuHttpClient;
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
pub fn setup(
    path: &std::path::Path,
    data: &mut ShareMap,
    announcers: &mut AnnouncerHandler,
) -> CommandResult {
    // Databases
    OsuSavedUsers::insert_into(&mut *data, &path.join("osu_saved_users.yaml"))?;
    OsuLastBeatmap::insert_into(&mut *data, &path.join("last_beatmaps.yaml"))?;

    // API client
    let http_client = data.get_cloned::<HTTPClient>();
    let osu_client = OsuHttpClient::new(
        http_client.clone(),
        std::env::var("OSU_API_KEY").expect("Please set OSU_API_KEY as osu! api key."),
    );
    data.insert::<OsuClient>(osu_client.clone());
    data.insert::<oppai_cache::BeatmapCache>(oppai_cache::BeatmapCache::new(http_client));
    data.insert::<beatmap_cache::BeatmapMetaCache>(beatmap_cache::BeatmapMetaCache::new(
        osu_client,
    ));

    // Announcer
    announcers.add(announcer::ANNOUNCER_KEY, announcer::updates);
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
pub fn std(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Std)
}

#[command]
#[aliases("osu!taiko")]
#[description = "Receive information about an user in osu!taiko mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub fn taiko(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Taiko)
}

#[command]
#[aliases("fruits", "osu!catch", "ctb")]
#[description = "Receive information about an user in osu!catch mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub fn catch(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Catch)
}

#[command]
#[aliases("osu!mania")]
#[description = "Receive information about an user in osu!mania mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub fn mania(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    get_user(ctx, msg, args, Mode::Mania)
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
pub fn save(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let osu = ctx.data.get_cloned::<OsuClient>();

    let user = args.single::<String>()?;
    let user: Option<User> = osu.user(UserID::Auto(user), |f| f)?;
    match user {
        Some(u) => {
            let db = OsuSavedUsers::open(&*ctx.data.read());
            let mut db = db.borrow_mut()?;

            db.insert(
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
            )?;
        }
        None => {
            msg.reply(&ctx, "user not found...")?;
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
    data: &ShareMap,
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

fn list_plays(plays: Vec<Score>, mode: Mode, ctx: Context, m: &Message) -> CommandResult {
    let watcher = ctx.data.get_cloned::<ReactionWatcher>();
    let osu = ctx.data.get_cloned::<BeatmapMetaCache>();
    let beatmap_cache = ctx.data.get_cloned::<BeatmapCache>();

    if plays.is_empty() {
        m.reply(&ctx, "No plays found")?;
        return Ok(());
    }

    let mut beatmaps: Vec<Option<String>> = vec![None; plays.len()];

    const ITEMS_PER_PAGE: usize = 5;
    let total_pages = (plays.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    watcher.paginate_fn(
        ctx,
        m.channel_id,
        move |page, e| {
            let page = page as usize;
            let start = page * ITEMS_PER_PAGE;
            let end = plays.len().min(start + ITEMS_PER_PAGE);
            if start >= end {
                return (e, Err(Error::from("No more pages")));
            }

            let plays = &plays[start..end];
            let beatmaps = {
                let b = &mut beatmaps[start..end];
                b.par_iter_mut()
                    .enumerate()
                    .map(|(i, v)| {
                        v.get_or_insert_with(|| {
                            if let Some(b) = osu.get_beatmap(plays[i].beatmap_id, mode).ok() {
                                let stars = beatmap_cache
                                    .get_beatmap(b.beatmap_id)
                                    .ok()
                                    .and_then(|b| {
                                        mode.to_oppai_mode().and_then(|mode| {
                                            b.get_info_with(Some(mode), plays[i].mods).ok()
                                        })
                                    })
                                    .map(|info| info.stars as f64)
                                    .unwrap_or(b.difficulty.stars);
                                format!(
                                    "[{:.1}*] {} - {} [{}] ({})",
                                    stars,
                                    b.artist,
                                    b.title,
                                    b.difficulty_name,
                                    b.short_link(Some(mode), Some(plays[i].mods)),
                                )
                            } else {
                                "FETCH_FAILED".to_owned()
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            };
            let pp = plays
                .iter()
                .map(|p| {
                    p.pp.map(|pp| format!("{:.2}pp", pp))
                        .or_else(|| {
                            beatmap_cache.get_beatmap(p.beatmap_id).ok().and_then(|b| {
                                mode.to_oppai_mode().and_then(|op| {
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
                            })
                        })
                        .unwrap_or("-".to_owned())
                })
                .collect::<Vec<_>>();
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
            (e.content(m.build()), Ok(()))
        },
        std::time::Duration::from_secs(60),
    )
}

#[command]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = --all] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[max_args(3)]
pub fn recent(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let mode = args.single::<ModeArg>().unwrap_or(ModeArg(Mode::Std)).0;
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;

    let osu = ctx.data.get_cloned::<OsuClient>();
    let meta_cache = ctx.data.get_cloned::<BeatmapMetaCache>();
    let oppai = ctx.data.get_cloned::<BeatmapCache>();
    let user = osu
        .user(user, |f| f.mode(mode))?
        .ok_or(Error::from("User not found"))?;
    match nth {
        Nth::Nth(nth) => {
            let recent_play = osu
                .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(nth))?
                .into_iter()
                .last()
                .ok_or(Error::from("No such play"))?;
            let beatmap = meta_cache
                .get_beatmap(recent_play.beatmap_id, mode)
                .unwrap();
            let content = oppai.get_beatmap(beatmap.beatmap_id)?;
            let beatmap_mode = BeatmapWithMode(beatmap, mode);

            msg.channel_id.send_message(&ctx, |m| {
                m.content(format!(
                    "{}: here is the play that you requested",
                    msg.author
                ))
                .embed(|m| score_embed(&recent_play, &beatmap_mode, &content, &user, None, m))
            })?;

            // Save the beatmap...
            cache::save_beatmap(&*ctx.data.read(), msg.channel_id, &beatmap_mode)?;
        }
        Nth::All => {
            let plays = osu.user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(50))?;
            list_plays(plays, mode, ctx.clone(), msg)?;
        }
    }
    Ok(())
}

#[command]
#[description = "Show information from the last queried beatmap."]
#[usage = "[mods = no mod]"]
#[max_args(1)]
pub fn last(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let b = cache::get_beatmap(&*ctx.data.read(), msg.channel_id)?;

    match b {
        Some(BeatmapWithMode(b, m)) => {
            let mods = args.find::<Mods>().unwrap_or(Mods::NOMOD);
            let info = ctx
                .data
                .get_cloned::<BeatmapCache>()
                .get_beatmap(b.beatmap_id)?
                .get_info_with(m.to_oppai_mode(), mods)
                .ok();
            msg.channel_id.send_message(&ctx, |f| {
                f.content(format!(
                    "{}: here is the beatmap you requested!",
                    msg.author
                ))
                .embed(|c| beatmap_embed(&b, m, mods, info, c))
            })?;
        }
        None => {
            msg.reply(&ctx, "No beatmap was queried on this channel.")?;
        }
    }

    Ok(())
}

#[command]
#[aliases("c", "chk")]
#[description = "Check your own or someone else's best record on the last beatmap."]
#[max_args(1)]
pub fn check(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let bm = cache::get_beatmap(&*ctx.data.read(), msg.channel_id)?;

    match bm {
        None => {
            msg.reply(&ctx, "No beatmap queried on this channel.")?;
        }
        Some(bm) => {
            let b = &bm.0;
            let m = bm.1;
            let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;

            let osu = ctx.data.get_cloned::<OsuClient>();
            let oppai = ctx.data.get_cloned::<BeatmapCache>();

            let content = oppai.get_beatmap(b.beatmap_id)?;

            let user = osu
                .user(user, |f| f)?
                .ok_or(Error::from("User not found"))?;
            let scores = osu.scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(m))?;

            if scores.is_empty() {
                msg.reply(&ctx, "No scores found")?;
            }

            for score in scores.into_iter() {
                msg.channel_id.send_message(&ctx, |c| {
                    c.embed(|m| score_embed(&score, &bm, &content, &user, None, m))
                })?;
            }
        }
    }

    Ok(())
}

#[command]
#[description = "Get the n-th top record of an user."]
#[usage = "#[n-th = --all] / [mode (std, taiko, catch, mania) = std / [username or user_id = your saved user id]"]
#[example = "#2 / taiko / natsukagami"]
#[max_args(3)]
pub fn top(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let mode = args
        .single::<ModeArg>()
        .map(|ModeArg(t)| t)
        .unwrap_or(Mode::Std);

    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;

    let osu = ctx.data.get_cloned::<OsuClient>();
    let oppai = ctx.data.get_cloned::<BeatmapCache>();
    let user = osu
        .user(user, |f| f.mode(mode))?
        .ok_or(Error::from("User not found"))?;

    match nth {
        Nth::Nth(nth) => {
            let top_play = osu.user_best(UserID::ID(user.id), |f| f.mode(mode).limit(nth))?;

            let rank = top_play.len() as u8;

            let top_play = top_play
                .into_iter()
                .last()
                .ok_or(Error::from("No such play"))?;
            let beatmap = osu
                .beatmaps(BeatmapRequestKind::Beatmap(top_play.beatmap_id), |f| {
                    f.mode(mode, true)
                })?
                .into_iter()
                .next()
                .unwrap();
            let content = oppai.get_beatmap(beatmap.beatmap_id)?;
            let beatmap = BeatmapWithMode(beatmap, mode);

            msg.channel_id.send_message(&ctx, |m| {
                m.content(format!(
                    "{}: here is the play that you requested",
                    msg.author
                ))
                .embed(|m| score_embed(&top_play, &beatmap, &content, &user, Some(rank), m))
            })?;

            // Save the beatmap...
            cache::save_beatmap(&*ctx.data.read(), msg.channel_id, &beatmap)?;
        }
        Nth::All => {
            let plays = osu.user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))?;
            list_plays(plays, mode, ctx.clone(), msg)?;
        }
    }
    Ok(())
}

fn get_user(ctx: &mut Context, msg: &Message, mut args: Args, mode: Mode) -> CommandResult {
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;
    let osu = ctx.data.get_cloned::<OsuClient>();
    let cache = ctx.data.get_cloned::<BeatmapMetaCache>();
    let user = osu.user(user, |f| f.mode(mode))?;
    let oppai = ctx.data.get_cloned::<BeatmapCache>();
    match user {
        Some(u) => {
            let best = osu
                .user_best(UserID::ID(u.id), |f| f.limit(1).mode(mode))?
                .into_iter()
                .next()
                .map(|m| -> Result<_, Error> {
                    let beatmap = cache.get_beatmap(m.beatmap_id, mode)?;
                    let info = mode
                        .to_oppai_mode()
                        .map(|mode| -> Result<_, Error> {
                            Ok(oppai
                                .get_beatmap(m.beatmap_id)?
                                .get_info_with(Some(mode), m.mods)?)
                        })
                        .transpose()?;
                    Ok((m, BeatmapWithMode(beatmap, mode), info))
                })
                .transpose()?;
            msg.channel_id.send_message(&ctx, |m| {
                m.content(format!(
                    "{}: here is the user that you requested",
                    msg.author
                ))
                .embed(|m| user_embed(u, best, m))
            })
        }
        None => msg.reply(&ctx, "🔍 user not found!"),
    }?;
    Ok(())
}

use crate::{
    models::{Beatmap, Mode, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuHttpClient,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::{channel::Message, id::UserId},
    utils::MessageBuilder,
};
use std::str::FromStr;
use youmubot_prelude::*;

mod announcer;
mod cache;
mod db;
pub(crate) mod embeds;
mod hook;
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
    data.insert::<OsuClient>(OsuHttpClient::new(
        http_client,
        std::env::var("OSU_API_KEY").expect("Please set OSU_API_KEY as osu! api key."),
    ));

    // Announcer
    announcers.add(announcer::ANNOUNCER_KEY, announcer::updates);
    Ok(())
}

#[group]
#[prefix = "osu"]
#[description = "osu! related commands."]
#[commands(std, taiko, catch, mania, save, recent, last, check, top, server_rank)]
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
    /// Whether this beatmap-with-mode is a converted beatmap.
    fn is_converted(&self) -> bool {
        self.0.mode != self.1
    }

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
        Ok(ModeArg(match s {
            "std" => Mode::Std,
            "taiko" => Mode::Taiko,
            "catch" => Mode::Catch,
            "mania" => Mode::Mania,
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
struct Nth(u8);

impl FromStr for Nth {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("#") {
            Err(Error::from("Not an order"))
        } else {
            let v = s.split_at("#".len()).1.parse()?;
            Ok(Nth(v))
        }
    }
}

#[command]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = 1] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[max_args(3)]
pub fn recent(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let nth = args.single::<Nth>().unwrap_or(Nth(1)).0.min(50).max(1);
    let mode = args.single::<ModeArg>().unwrap_or(ModeArg(Mode::Std)).0;
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;

    let osu = ctx.data.get_cloned::<OsuClient>();
    let user = osu
        .user(user, |f| f.mode(mode))?
        .ok_or(Error::from("User not found"))?;
    let recent_play = osu
        .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(nth))?
        .into_iter()
        .last()
        .ok_or(Error::from("No such play"))?;
    let beatmap = osu
        .beatmaps(BeatmapRequestKind::Beatmap(recent_play.beatmap_id), |f| {
            f.mode(mode, true)
        })?
        .into_iter()
        .next()
        .map(|v| BeatmapWithMode(v, mode))
        .unwrap();

    msg.channel_id.send_message(&ctx, |m| {
        m.content(format!(
            "{}: here is the play that you requested",
            msg.author
        ))
        .embed(|m| score_embed(&recent_play, &beatmap, &user, None, m))
    })?;

    // Save the beatmap...
    cache::save_beatmap(&*ctx.data.read(), msg.channel_id, &beatmap)?;

    Ok(())
}

#[command]
#[description = "Show information from the last queried beatmap."]
#[num_args(0)]
pub fn last(ctx: &mut Context, msg: &Message, _: Args) -> CommandResult {
    let b = cache::get_beatmap(&*ctx.data.read(), msg.channel_id)?;

    match b {
        Some(BeatmapWithMode(b, m)) => {
            msg.channel_id.send_message(&ctx, |f| {
                f.content(format!(
                    "{}: here is the beatmap you requested!",
                    msg.author
                ))
                .embed(|c| beatmap_embed(&b, m, c))
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

            let user = osu
                .user(user, |f| f)?
                .ok_or(Error::from("User not found"))?;
            let scores = osu.scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(m))?;

            if scores.is_empty() {
                msg.reply(&ctx, "No scores found")?;
            }

            for score in scores.into_iter() {
                msg.channel_id.send_message(&ctx, |c| {
                    c.embed(|m| score_embed(&score, &bm, &user, None, m))
                })?;
            }
        }
    }

    Ok(())
}

#[command]
#[description = "Get the n-th top record of an user."]
#[usage = "#[n-th = 1] / [mode (std, taiko, catch, mania) = std / [username or user_id = your saved user id]"]
#[example = "#2 / taiko / natsukagami"]
#[max_args(3)]
pub fn top(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let nth = args.single::<Nth>().unwrap_or(Nth(1)).0;
    let mode = args
        .single::<ModeArg>()
        .map(|ModeArg(t)| t)
        .unwrap_or(Mode::Std);

    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;

    let osu = ctx.data.get_cloned::<OsuClient>();
    let user = osu
        .user(user, |f| f.mode(mode))?
        .ok_or(Error::from("User not found"))?;
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
        .map(|v| BeatmapWithMode(v, mode))
        .unwrap();

    msg.channel_id.send_message(&ctx, |m| {
        m.content(format!(
            "{}: here is the play that you requested",
            msg.author
        ))
        .embed(|m| score_embed(&top_play, &beatmap, &user, Some(rank), m))
    })?;

    // Save the beatmap...
    cache::save_beatmap(&*ctx.data.read(), msg.channel_id, &beatmap)?;

    Ok(())
}

fn get_user(ctx: &mut Context, msg: &Message, mut args: Args, mode: Mode) -> CommandResult {
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &*ctx.data.read(), msg)?;
    let osu = ctx.data.get_cloned::<OsuClient>();
    let user = osu.user(user, |f| f.mode(mode))?;
    match user {
        Some(u) => {
            let best = osu
                .user_best(UserID::ID(u.id), |f| f.limit(1).mode(mode))?
                .into_iter()
                .next()
                .map(|m| {
                    osu.beatmaps(BeatmapRequestKind::Beatmap(m.beatmap_id), |f| {
                        f.mode(mode, true)
                    })
                    .map(|map| (m, BeatmapWithMode(map.into_iter().next().unwrap(), mode)))
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
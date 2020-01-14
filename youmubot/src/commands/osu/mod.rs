use crate::db::{DBWriteGuard, OsuSavedUsers};
use crate::http;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    prelude::*,
    utils::MessageBuilder,
};
use std::str::FromStr;
use youmubot_osu::{
    models::{Beatmap, Mode, Rank, Score, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuClient,
};

mod cache;
pub(crate) mod embeds;
mod hook;

use embeds::{beatmap_embed, score_embed, user_embed};
pub use hook::hook;

group!({
    name: "osu",
    options: {
        prefix: "osu",
        description: "osu! related commands.",
    },
    commands: [std, taiko, catch, mania, save, recent, last],
});

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
    let mut data = ctx.data.write();
    let reqwest = data.get::<http::HTTP>().unwrap();
    let osu = data.get::<http::Osu>().unwrap();

    let user = args.single::<String>()?;
    let user: Option<User> = osu.user(reqwest, UserID::Auto(user), |f| f)?;
    match user {
        Some(u) => {
            let mut db: DBWriteGuard<_> = data
                .get_mut::<OsuSavedUsers>()
                .ok_or(Error::from("DB uninitialized"))?
                .into();
            let mut db = db.borrow_mut()?;

            db.insert(msg.author.id, u.id);
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
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ModeArg(match s {
            "std" => Mode::Std,
            "taiko" => Mode::Taiko,
            "catch" => Mode::Catch,
            "mania" => Mode::Mania,
            _ => return Err(Error::from(format!("Unknown mode {}", s))),
        }))
    }
}

#[command]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = 1] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[max_args(3)]
pub fn recent(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
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

    let mut data = ctx.data.write();

    let nth = args.single::<Nth>().unwrap_or(Nth(1)).0.min(50).max(1);
    let mode = args.single::<ModeArg>().unwrap_or(ModeArg(Mode::Std)).0;
    let user = match args.single::<String>() {
        Ok(v) => v,
        Err(_) => {
            let db: DBWriteGuard<_> = data
                .get_mut::<OsuSavedUsers>()
                .ok_or(Error::from("DB uninitialized"))?
                .into();
            let db = db.borrow()?;
            match db.get(&msg.author.id) {
                Some(ref v) => v.to_string(),
                None => {
                    msg.reply(&ctx, "You have not saved any account.")?;
                    return Ok(());
                }
            }
        }
    };

    let reqwest = data.get::<http::HTTP>().unwrap();
    let osu: &OsuClient = data.get::<http::Osu>().unwrap();
    let user = osu
        .user(reqwest, UserID::Auto(user), |f| f.mode(mode))?
        .ok_or(Error::from("User not found"))?;
    let recent_play = osu
        .user_recent(reqwest, UserID::ID(user.id), |f| f.mode(mode).limit(nth))?
        .into_iter()
        .last()
        .ok_or(Error::from("No such play"))?;
    let beatmap = osu
        .beatmaps(
            reqwest,
            BeatmapRequestKind::Beatmap(recent_play.beatmap_id),
            |f| f.mode(mode, true),
        )?
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
    cache::save_beatmap(&mut *data, msg.channel_id, &beatmap)?;

    Ok(())
}

#[command]
#[description = "Show information from the last queried beatmap."]
#[num_args(0)]
pub fn last(ctx: &mut Context, msg: &Message, _: Args) -> CommandResult {
    let mut data = ctx.data.write();

    let b = cache::get_beatmap(&mut *data, msg.channel_id)?;

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

fn get_user(ctx: &mut Context, msg: &Message, args: Args, mode: Mode) -> CommandResult {
    let mut data = ctx.data.write();
    let username = match args.remains() {
        Some(v) => v.to_owned(),
        None => {
            let db: DBWriteGuard<_> = data
                .get_mut::<OsuSavedUsers>()
                .ok_or(Error::from("DB uninitialized"))?
                .into();
            let db = db.borrow()?;
            match db.get(&msg.author.id) {
                Some(ref v) => v.to_string(),
                None => {
                    msg.reply(&ctx, "You have not saved any account.")?;
                    return Ok(());
                }
            }
        }
    };
    let reqwest = data.get::<http::HTTP>().unwrap();
    let osu = data.get::<http::Osu>().unwrap();
    let user = osu.user(reqwest, UserID::Auto(username), |f| f.mode(mode))?;
    match user {
        Some(u) => {
            let best = osu
                .user_best(reqwest, UserID::ID(u.id), |f| f.limit(1).mode(mode))?
                .into_iter()
                .next()
                .map(|m| {
                    osu.beatmaps(reqwest, BeatmapRequestKind::Beatmap(m.beatmap_id), |f| {
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

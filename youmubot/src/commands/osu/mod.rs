use crate::commands::args::Duration;
use crate::db::{DBWriteGuard, OsuSavedUsers};
use crate::http;
use chrono::Utc;
use serenity::{
    builder::CreateEmbed,
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    prelude::*,
    utils::MessageBuilder,
};
use youmubot_osu::{
    models::{Beatmap, Mode, Rank, Score, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuClient,
};

mod cache;
mod hook;

pub use hook::hook;
use std::str::FromStr;

group!({
    name: "osu",
    options: {
        prefix: "osu",
        description: "osu! related commands.",
    },
    commands: [std, taiko, catch, mania, save, recent],
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

fn score_embed<'a>(
    s: &Score,
    bm: &BeatmapWithMode,
    u: &User,
    top_record: Option<u8>,
    m: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let mode = bm.mode();
    let b = &bm.0;
    let accuracy = s.accuracy(mode);
    let score_line = match &s.rank {
        Rank::SS | Rank::SSH => format!("SS"),
        _ if s.perfect => format!("{:2}% FC", accuracy),
        Rank::F => format!("{:.2}% {} combo [FAILED]", accuracy, s.max_combo),
        v => format!("{:.2}% {} combo {} rank", accuracy, s.max_combo, v),
    };
    let score_line =
        s.pp.map(|pp| format!("{} | {:2}pp", &score_line, pp))
            .unwrap_or(score_line);
    let top_record = top_record
        .map(|v| format!("| #{} top record!", v))
        .unwrap_or("".to_owned());
    m.author(|f| f.name(&u.username).url(u.link()).icon_url(u.avatar_url()))
        .color(0xffb6c1)
        .title(format!(
            "{} | {} - {} [{}] {} ({:.2}\\*) by {} | {} {}",
            u.username,
            b.artist,
            b.title,
            b.difficulty_name,
            s.mods,
            b.difficulty.stars,
            b.creator,
            score_line,
            top_record
        ))
        .description(format!("[[Beatmap]]({})", b.link()))
        .image(b.cover_url())
        .field(
            "Beatmap",
            format!("{} - {} [{}]", b.artist, b.title, b.difficulty_name),
            false,
        )
        .field("Rank", &score_line, false)
        .fields(s.pp.map(|pp| ("pp gained", format!("{:2}pp", pp), true)))
        .field("Creator", &b.creator, true)
        .field("Mode", mode.to_string(), true)
        .field(
            "Map stats",
            MessageBuilder::new()
                .push(format!("[[Link]]({})", b.link()))
                .push(", ")
                .push_bold(format!("{:.2}⭐", b.difficulty.stars))
                .push(", ")
                .push_bold_line(
                    b.mode.to_string()
                        + if bm.is_converted() {
                            ""
                        } else {
                            " (Converted)"
                        },
                )
                .push("CS")
                .push_bold(format!("{:.1}", b.difficulty.cs))
                .push(", AR")
                .push_bold(format!("{:.1}", b.difficulty.ar))
                .push(", OD")
                .push_bold(format!("{:.1}", b.difficulty.od))
                .push(", HP")
                .push_bold(format!("{:.1}", b.difficulty.hp))
                .push(", ⌛ ")
                .push_bold(format!("{}", Duration(b.drain_length)))
                .build(),
            false,
        )
        .field("Played on", s.date.format("%F %T"), false)
}

fn user_embed<'a>(
    u: User,
    best: Option<(Score, BeatmapWithMode)>,
    m: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    m.title(u.username)
        .url(format!("https://osu.ppy.sh/users/{}", u.id))
        .color(0xffb6c1)
        .thumbnail(format!("https://a.ppy.sh/{}", u.id))
        .description(format!("Member since **{}**", u.joined.format("%F %T")))
        .field(
            "Performance Points",
            u.pp.map(|v| format!("{:.2}pp", v))
                .unwrap_or("Inactive".to_owned()),
            false,
        )
        .field("World Rank", format!("#{}", u.rank), true)
        .field(
            "Country Rank",
            format!(":flag_{}: #{}", u.country.to_lowercase(), u.country_rank),
            true,
        )
        .field("Accuracy", format!("{:.2}%", u.accuracy), true)
        .field(
            "Play count",
            format!("{} (play time: {})", u.play_count, Duration(u.played_time)),
            false,
        )
        .field(
            "Ranks",
            format!(
                "{} SSH | {} SS | {} SH | {} S | {} A",
                u.count_ssh, u.count_ss, u.count_sh, u.count_s, u.count_a
            ),
            false,
        )
        .field(
            "Level",
            format!(
                "Level **{:.0}**: {} total score, {} ranked score",
                u.level, u.total_score, u.ranked_score
            ),
            false,
        )
        .fields(best.map(|(v, map)| {
            let map = map.0;
            (
                "Best Record",
                MessageBuilder::new()
                    .push_bold(format!(
                        "{:.2}pp",
                        v.pp.unwrap() /*Top record should have pp*/
                    ))
                    .push(" - ")
                    .push_line(format!(
                        "{:.1} ago",
                        Duration(
                            (Utc::now() - v.date)
                                .to_std()
                                .unwrap_or(std::time::Duration::from_secs(1))
                        )
                    ))
                    .push("on ")
                    .push(format!(
                        "[{} - {}]({})",
                        MessageBuilder::new().push_bold_safe(&map.artist).build(),
                        MessageBuilder::new().push_bold_safe(&map.title).build(),
                        map.link()
                    ))
                    .push(format!(" [{}]", map.difficulty_name))
                    .push(format!(" ({:.1}⭐)", map.difficulty.stars))
                    .build(),
                false,
            )
        }))
}

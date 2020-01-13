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
    models::{Beatmap, Mode, Score, User},
    request::{BeatmapRequestKind, UserID},
};

mod hook;

pub use hook::hook;

group!({
    name: "osu",
    options: {
        prefix: "osu",
        description: "osu! related commands.",
    },
    commands: [std, taiko, catch, mania, save],
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
                    osu.beatmaps(reqwest, BeatmapRequestKind::Beatmap(m.beatmap_id), |f| f)
                        .map(|map| (m, map.into_iter().next().unwrap()))
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

fn user_embed<'a>(
    u: User,
    best: Option<(Score, Beatmap)>,
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
            (
                "Best Record",
                MessageBuilder::new()
                    .push_bold(format!("{:.2}pp", v.pp))
                    .push(" - ")
                    .push_line(format!("{:.1} ago", Duration(Utc::now() - v.date)))
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

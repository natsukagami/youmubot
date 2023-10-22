use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::display::ScoreListStyle,
    discord::oppai_cache::{BeatmapCache, BeatmapInfo},
    models::{Beatmap, Mode, Mods, User},
    request::{BeatmapRequestKind, UserID},
    Client as OsuHttpClient,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
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
pub(crate) mod display;
pub(crate) mod embeds;
mod hook;
pub(crate) mod oppai_cache;
mod register_user;
mod server_rank;

use db::OsuUser;
use db::{OsuLastBeatmap, OsuSavedUsers, OsuUserBests};
use embeds::{beatmap_embed, score_embed, user_embed};
use hook::SHORT_LINK_REGEX;
pub use hook::{dot_osu_hook, hook};
use server_rank::{SERVER_RANK_COMMAND, UPDATE_LEADERBOARD_COMMAND};

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
pub fn setup(
    _path: &std::path::Path,
    data: &mut TypeMap,
    announcers: &mut AnnouncerHandler,
) -> CommandResult {
    let sql_client = data.get::<SQLClient>().unwrap().clone();
    // Databases
    data.insert::<OsuSavedUsers>(OsuSavedUsers::new(sql_client.clone()));
    data.insert::<OsuLastBeatmap>(OsuLastBeatmap::new(sql_client.clone()));
    data.insert::<OsuUserBests>(OsuUserBests::new(sql_client.clone()));

    // Locks
    data.insert::<server_rank::update_lock::UpdateLock>(
        server_rank::update_lock::UpdateLock::default(),
    );

    // API client
    let http_client = data.get::<HTTPClient>().unwrap().clone();
    let make_client = || {
        OsuHttpClient::new(
            std::env::var("OSU_API_KEY").expect("Please set OSU_API_KEY as osu! api key."),
        )
    };
    let osu_client = Arc::new(make_client());
    data.insert::<OsuClient>(osu_client.clone());
    data.insert::<oppai_cache::BeatmapCache>(oppai_cache::BeatmapCache::new(
        http_client,
        sql_client.clone(),
    ));
    data.insert::<beatmap_cache::BeatmapMetaCache>(beatmap_cache::BeatmapMetaCache::new(
        osu_client, sql_client,
    ));

    // Announcer
    announcers.add(
        announcer::ANNOUNCER_KEY,
        announcer::Announcer::new(make_client()),
    );
    Ok(())
}

#[group]
#[prefix = "osu"]
#[description = "osu! related commands."]
#[commands(
    std,
    taiko,
    catch,
    mania,
    save,
    forcesave,
    recent,
    last,
    check,
    top,
    server_rank,
    update_leaderboard
)]
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
            let check_beatmap_id = register_user::user_register_beatmap_id(&u);
            let check = osu
                .user_recent(UserID::ID(u.id), |f| f.mode(Mode::Std).limit(1))
                .await?
                .into_iter()
                .take(1)
                .any(|s| s.beatmap_id == check_beatmap_id);
            if !check {
                let msg = msg.reply(&ctx, format!("To set your osu username, please make your most recent play be the following map: `/b/{}` in **osu! standard** mode! It does **not** have to be a pass.", check_beatmap_id));
                let beatmap = osu
                    .beatmaps(
                        crate::request::BeatmapRequestKind::Beatmap(check_beatmap_id),
                        |f| f,
                    )
                    .await?
                    .into_iter()
                    .next()
                    .unwrap();
                let info = data
                    .get::<BeatmapCache>()
                    .unwrap()
                    .get_beatmap(beatmap.beatmap_id)
                    .await?
                    .get_possible_pp_with(Mode::Std, Mods::NOMOD)?;
                msg.await?
                    .edit(&ctx, |f| {
                        f.embed(|e| beatmap_embed(&beatmap, Mode::Std, Mods::NOMOD, info, e))
                    })
                    .await?;
                return Ok(());
            }
            add_user(msg.author.id, u.id, &data).await?;
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

#[command]
#[description = "Save the given username as someone's username."]
#[owners_only]
#[usage = "[ping user]/[username or user_id]"]
#[delimiters(" ")]
#[num_args(2)]
pub async fn forcesave(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let osu = data.get::<OsuClient>().unwrap();
    let target = args.single::<serenity::model::id::UserId>()?;

    let user = args.quoted().trimmed().single::<String>()?;
    let user: Option<User> = osu.user(UserID::Auto(user), |f| f).await?;
    match user {
        Some(u) => {
            add_user(target, u.id, &data).await?;
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

async fn add_user(target: serenity::model::id::UserId, user_id: u64, data: &TypeMap) -> Result<()> {
    let u = OsuUser {
        user_id: target,
        id: user_id,
        failures: 0,
        last_update: chrono::Utc::now(),
        pp: [None, None, None, None],
    };
    data.get::<OsuSavedUsers>().unwrap().new_user(u).await?;
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

async fn to_user_id_query(
    s: Option<UsernameArg>,
    data: &TypeMap,
    msg: &Message,
) -> Result<UserID, Error> {
    let id = match s {
        Some(UsernameArg::Raw(s)) => return Ok(UserID::Auto(s)),
        Some(UsernameArg::Tagged(r)) => r,
        None => msg.author.id,
    };

    data.get::<OsuSavedUsers>()
        .unwrap()
        .by_user_id(id)
        .await?
        .map(|u| UserID::ID(u.id))
        .ok_or_else(|| Error::msg("No saved account found"))
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
        } else if !s.starts_with('#') {
            Err(Error::msg("Not an order"))
        } else {
            let v = s.split_at("#".len()).1.parse()?;
            Ok(Nth::Nth(v))
        }
    }
}

#[command]
#[aliases("rs", "rc", "r")]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = --all] / [style (table or grid) = --table] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[delimiters("/", " ")]
#[max_args(4)]
pub async fn recent(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let style = args.single::<ScoreListStyle>().unwrap_or_default();
    let mode = args.single::<ModeArg>().unwrap_or(ModeArg(Mode::Std)).0;
    let user = to_user_id_query(
        args.quoted().trimmed().single::<UsernameArg>().ok(),
        &data,
        msg,
    )
    .await?;

    let osu = data.get::<OsuClient>().unwrap();
    let meta_cache = data.get::<BeatmapMetaCache>().unwrap();
    let oppai = data.get::<BeatmapCache>().unwrap();
    let user = osu
        .user(user, |f| f.mode(mode))
        .await?
        .ok_or_else(|| Error::msg("User not found"))?;
    match nth {
        Nth::Nth(nth) => {
            let recent_play = osu
                .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(nth))
                .await?
                .into_iter()
                .last()
                .ok_or_else(|| Error::msg("No such play"))?;
            let beatmap = meta_cache.get_beatmap(recent_play.beatmap_id, mode).await?;
            let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(&ctx, |m| {
                    m.content("Here is the play that you requested".to_string())
                        .embed(|m| {
                            score_embed(&recent_play, &beatmap_mode, &content, &user).build(m)
                        })
                        .reference_message(msg)
                })
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&data, msg.channel_id, &beatmap_mode).await?;
        }
        Nth::All => {
            let plays = osu
                .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(50))
                .await?;
            style.display_scores(plays, mode, ctx, msg).await?;
        }
    }
    Ok(())
}

/// Get beatmapset.
struct OptBeatmapset;

impl FromStr for OptBeatmapset {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--set" | "-s" | "--beatmapset" => Ok(Self),
            _ => Err(Error::msg("not opt beatmapset")),
        }
    }
}

/// Load the mentioned beatmap from the given message.
pub(crate) async fn load_beatmap(
    ctx: &Context,
    msg: &Message,
) -> Option<(BeatmapWithMode, Option<Mods>)> {
    let data = ctx.data.read().await;

    if let Some(replied) = &msg.referenced_message {
        // Try to look for a mention of the replied message.
        let beatmap_id = SHORT_LINK_REGEX.captures(&replied.content).or_else(|| {
            replied.embeds.iter().find_map(|e| {
                e.description
                    .as_ref()
                    .and_then(|v| SHORT_LINK_REGEX.captures(v))
                    .or_else(|| {
                        e.fields
                            .iter()
                            .find_map(|f| SHORT_LINK_REGEX.captures(&f.value))
                    })
            })
        });
        if let Some(caps) = beatmap_id {
            let id: u64 = caps.name("id").unwrap().as_str().parse().unwrap();
            let mode = caps
                .name("mode")
                .and_then(|m| Mode::parse_from_new_site(m.as_str()));
            let mods = caps
                .name("mods")
                .and_then(|m| m.as_str().parse::<Mods>().ok());
            let osu = data.get::<OsuClient>().unwrap();
            let bms = osu
                .beatmaps(BeatmapRequestKind::Beatmap(id), |f| f.maybe_mode(mode))
                .await
                .ok()
                .and_then(|v| v.into_iter().next());
            if let Some(beatmap) = bms {
                let bm_mode = beatmap.mode;
                let bm = BeatmapWithMode(beatmap, mode.unwrap_or(bm_mode));
                // Store the beatmap in history
                cache::save_beatmap(&data, msg.channel_id, &bm)
                    .await
                    .pls_ok();

                return Some((bm, mods));
            }
        }
    }

    let b = cache::get_beatmap(&data, msg.channel_id)
        .await
        .ok()
        .flatten();
    b.map(|b| (b, None))
}

#[command]
#[aliases("map")]
#[description = "Show information from the last queried beatmap."]
#[usage = "[--set/-s/--beatmapset] / [mods = no mod]"]
#[delimiters(" ")]
#[max_args(2)]
pub async fn last(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let b = load_beatmap(ctx, msg).await;
    let beatmapset = args.find::<OptBeatmapset>().is_ok();

    match b {
        Some((BeatmapWithMode(b, m), mods_def)) => {
            let mods = args.find::<Mods>().ok().or(mods_def).unwrap_or(Mods::NOMOD);
            if beatmapset {
                let beatmap_cache = data.get::<BeatmapMetaCache>().unwrap();
                let beatmapset = beatmap_cache.get_beatmapset(b.beatmapset_id).await?;
                display::display_beatmapset(
                    ctx,
                    beatmapset,
                    None,
                    Some(mods),
                    msg,
                    "Here is the beatmapset you requested!",
                )
                .await?;
                return Ok(());
            }
            let info = data
                .get::<BeatmapCache>()
                .unwrap()
                .get_beatmap(b.beatmap_id)
                .await?
                .get_possible_pp_with(m, mods)?;
            msg.channel_id
                .send_message(&ctx, |f| {
                    f.content("Here is the beatmap you requested!")
                        .embed(|c| beatmap_embed(&b, m, mods, info, c))
                        .reference_message(msg)
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
#[usage = "[style (table or grid) = --table] / [username or tag = yourself] / [mods to filter]"]
#[description = "Check your own or someone else's best record on the last beatmap. Also stores the result if possible."]
#[max_args(3)]
pub async fn check(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let bm = load_beatmap(ctx, msg).await;

    match bm {
        None => {
            msg.reply(&ctx, "No beatmap queried on this channel.")
                .await?;
        }
        Some((bm, mods_def)) => {
            let mods = args.find::<Mods>().ok().or(mods_def).unwrap_or(Mods::NOMOD);
            let b = &bm.0;
            let m = bm.1;
            let style = args
                .single::<ScoreListStyle>()
                .unwrap_or(ScoreListStyle::Grid);
            let username_arg = args.single::<UsernameArg>().ok();
            let user_id = match username_arg.as_ref() {
                Some(UsernameArg::Tagged(v)) => Some(*v),
                None => Some(msg.author.id),
                _ => None,
            };
            let user = to_user_id_query(username_arg, &data, msg).await?;

            let osu = data.get::<OsuClient>().unwrap();

            let user = osu
                .user(user, |f| f)
                .await?
                .ok_or_else(|| Error::msg("User not found"))?;
            let mut scores = osu
                .scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(m))
                .await?
                .into_iter()
                .filter(|s| s.mods.contains(mods))
                .collect::<Vec<_>>();
            scores.sort_by(|a, b| b.pp.unwrap().partial_cmp(&a.pp.unwrap()).unwrap());

            if scores.is_empty() {
                msg.reply(&ctx, "No scores found").await?;
                return Ok(());
            }

            if let Some(user_id) = user_id {
                // Save to database
                data.get::<OsuUserBests>()
                    .unwrap()
                    .save(user_id, m, scores.clone())
                    .await
                    .pls_ok();
            }

            style.display_scores(scores, m, ctx, msg).await?;
        }
    }

    Ok(())
}

#[command]
#[aliases("t")]
#[description = "Get the n-th top record of an user."]
#[usage = "#[n-th = --all] / [style (table or grid) = --table] / [mode (std, taiko, catch, mania)] = std / [username or user_id = your saved user id]"]
#[example = "#2 / taiko / natsukagami"]
#[max_args(4)]
pub async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let nth = args.single::<Nth>().unwrap_or(Nth::All);
    let style = args.single::<ScoreListStyle>().unwrap_or_default();
    let mode = args
        .single::<ModeArg>()
        .map(|ModeArg(t)| t)
        .unwrap_or(Mode::Std);

    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &data, msg).await?;
    let meta_cache = data.get::<BeatmapMetaCache>().unwrap();
    let osu = data.get::<OsuClient>().unwrap();

    let oppai = data.get::<BeatmapCache>().unwrap();
    let user = osu
        .user(user, |f| f.mode(mode))
        .await?
        .ok_or_else(|| Error::msg("User not found"))?;

    match nth {
        Nth::Nth(nth) => {
            let top_play = osu
                .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(nth))
                .await?;

            let rank = top_play.len() as u8;

            let top_play = top_play
                .into_iter()
                .last()
                .ok_or_else(|| Error::msg("No such play"))?;
            let beatmap = meta_cache.get_beatmap(top_play.beatmap_id, mode).await?;
            let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(&ctx, |m| {
                    m.content(format!(
                        "{}: here is the play that you requested",
                        msg.author
                    ))
                    .embed(|m| {
                        score_embed(&top_play, &beatmap, &content, &user)
                            .top_record(rank)
                            .build(m)
                    })
                })
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&data, msg.channel_id, &beatmap).await?;
        }
        Nth::All => {
            let plays = osu
                .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))
                .await?;
            style.display_scores(plays, mode, ctx, msg).await?;
        }
    }
    Ok(())
}

async fn get_user(ctx: &Context, msg: &Message, mut args: Args, mode: Mode) -> CommandResult {
    let data = ctx.data.read().await;
    let user = to_user_id_query(args.single::<UsernameArg>().ok(), &data, msg).await?;
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
                    let info = oppai
                        .get_beatmap(m.beatmap_id)
                        .await?
                        .get_info_with(mode, m.mods)?;
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

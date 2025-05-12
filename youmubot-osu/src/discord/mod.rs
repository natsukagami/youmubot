use std::{borrow::Borrow, collections::HashMap as Map, str::FromStr, sync::Arc};

use chrono::Utc;
use futures_util::join;

use interaction::{beatmap_components, score_components};
use link_parser::EmbedType;
use oppai_cache::BeatmapInfoWithPP;
use rand::seq::IteratorRandom;
use serenity::{
    all::{CreateActionRow, CreateButton},
    builder::CreateMessage,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
};

pub use commands::osu as osu_command;
use db::{OsuLastBeatmap, OsuSavedUsers, OsuUser, OsuUserMode};
use embeds::{beatmap_embed, score_embed, user_embed};
pub use hook::{dot_osu_hook, hook, score_hook};
use server_rank::{SERVER_RANK_COMMAND, SHOW_LEADERBOARD_COMMAND};
use stream::{FuturesOrdered, FuturesUnordered};
use youmubot_prelude::announcer::AnnouncerHandler;
use youmubot_prelude::*;

use crate::{
    discord::{
        beatmap_cache::BeatmapMetaCache,
        display::ScoreListStyle,
        oppai_cache::{BeatmapCache, BeatmapInfo},
    },
    models::{Beatmap, Mode, Mods, Score, User},
    mods::UnparsedMods,
    request::{BeatmapRequestKind, UserID},
    scores::Scores,
    OsuClient as OsuHttpClient, UserHeader, MAX_TOP_SCORES_INDEX,
};

mod announcer;
pub(crate) mod beatmap_cache;
mod cache;
mod commands;
mod db;
pub(crate) mod display;
pub(crate) mod embeds;
mod hook;
pub mod interaction;
mod link_parser;
pub(crate) mod oppai_cache;
mod server_rank;

/// The osu! client.
pub(crate) struct OsuClient;

impl TypeMapKey for OsuClient {
    type Value = Arc<crate::OsuClient>;
}

/// The environment for osu! app commands.
#[derive(Clone)]
pub struct OsuEnv {
    pub(crate) prelude: Env,
    // databases
    pub(crate) saved_users: OsuSavedUsers,
    pub(crate) last_beatmaps: OsuLastBeatmap,
    // clients
    pub(crate) client: Arc<crate::OsuClient>,
    pub(crate) oppai: BeatmapCache,
    pub(crate) beatmaps: BeatmapMetaCache,
}

/// Gets an [OsuEnv] from the current environment.
pub trait HasOsuEnv: Send + Sync {
    fn osu_env(&self) -> &OsuEnv;
}

impl<T: AsRef<OsuEnv> + Send + Sync> HasOsuEnv for T {
    fn osu_env(&self) -> &OsuEnv {
        self.as_ref()
    }
}

impl std::fmt::Debug for OsuEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<osu::Env>")
    }
}

impl TypeMapKey for OsuEnv {
    type Value = OsuEnv;
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
pub async fn setup(
    data: &mut TypeMap,
    prelude: youmubot_prelude::Env,
    announcers: &mut AnnouncerHandler,
) -> Result<OsuEnv> {
    // Databases
    let saved_users = OsuSavedUsers::new(prelude.sql.clone());
    let last_beatmaps = OsuLastBeatmap::new(prelude.sql.clone());

    // API client
    let osu_client = Arc::new(
        OsuHttpClient::new(
            std::env::var("OSU_API_CLIENT_ID")
                .expect("Please set OSU_API_CLIENT_ID as osu! api v2 client ID.")
                .parse()
                .expect("client_id should be u64"),
            std::env::var("OSU_API_CLIENT_SECRET")
                .expect("Please set OSU_API_CLIENT_SECRET as osu! api v2 client secret."),
        )
        .await
        .expect("osu! should be initialized"),
    );
    let oppai_cache = BeatmapCache::new(prelude.http.clone(), prelude.sql.clone());
    let beatmap_cache = BeatmapMetaCache::new(osu_client.clone(), prelude.sql.clone());

    // Announcer
    announcers.add(announcer::ANNOUNCER_KEY, announcer::Announcer::new());

    // Legacy data
    data.insert::<OsuLastBeatmap>(last_beatmaps.clone());
    data.insert::<OsuSavedUsers>(saved_users.clone());
    data.insert::<OsuClient>(osu_client.clone());
    data.insert::<BeatmapCache>(oppai_cache.clone());
    data.insert::<BeatmapMetaCache>(beatmap_cache.clone());

    let env = OsuEnv {
        prelude,
        saved_users,
        last_beatmaps,
        client: osu_client,
        oppai: oppai_cache,
        beatmaps: beatmap_cache,
    };

    data.insert::<OsuEnv>(env.clone());

    Ok(env)
}

#[group]
#[prefix = "osu"]
#[description = "osu! related commands."]
#[commands(
    user,
    std,
    taiko,
    catch,
    mania,
    save,
    forcesave,
    recent,
    pins,
    last,
    check,
    top,
    server_rank,
    show_leaderboard,
    clean_cache
)]
#[default_command(user)]

struct Osu;

#[command]
#[description = "Receive information about an user in their preferred mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn user(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    get_user(ctx, &env, msg, args, None).await
}

#[command]
#[aliases("osu", "osu!")]
#[description = "Receive information about an user in osu!std mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn std(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    get_user(ctx, &env, msg, args, Mode::Std).await
}

#[command]
#[aliases("osu!taiko")]
#[description = "Receive information about an user in osu!taiko mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn taiko(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    get_user(ctx, &env, msg, args, Mode::Taiko).await
}

#[command]
#[aliases("fruits", "osu!catch", "ctb")]
#[description = "Receive information about an user in osu!catch mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn catch(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    get_user(ctx, &env, msg, args, Mode::Catch).await
}

#[command]
#[aliases("osu!mania")]
#[description = "Receive information about an user in osu!mania mode."]
#[usage = "[username or user_id = your saved username]"]
#[max_args(1)]
pub async fn mania(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    get_user(ctx, &env, msg, args, Mode::Mania).await
}

#[derive(Debug, Clone)]
pub(crate) struct BeatmapWithMode(pub Beatmap, pub Option<Mode>);

impl BeatmapWithMode {
    fn mode(&self) -> Mode {
        self.1.unwrap_or(self.0.mode)
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
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let user = args.single::<String>()?;

    let (u, mode, score, beatmap, info) = find_save_requirements(&env, user).await?;
    let reply = msg
        .channel_id
        .send_message(
            &ctx,
            CreateMessage::new()
                .content(save_request_message(&u.username, score.beatmap_id, mode))
                .embed(beatmap_embed(&beatmap, mode, Mods::NOMOD, &info))
                .components(vec![beatmap_components(mode, msg.guild_id), save_button()]),
        )
        .await?;
    let mut p = (reply, ctx);
    match handle_save_respond(ctx, &env, msg.author.id, &mut p, &beatmap, u, mode).await {
        Ok(_) => (),
        Err(e) => {
            p.0.delete(&ctx).await?;
            return Err(e.into());
        }
    };
    Ok(())
}

pub(crate) fn save_request_message(username: &str, beatmap_id: u64, mode: Mode) -> String {
    format!(
        "To set your osu username to **{}**, please make your most recent play \
            be the following map: `/b/{}` in **{}** mode! \
        It does **not** have to be a pass, and **NF** can be used! \
        React to this message with 👌 within 5 minutes when you're done!",
        username,
        beatmap_id,
        mode.as_str_new_site()
    )
}

pub(crate) async fn find_save_requirements(
    env: &OsuEnv,
    username: String,
) -> Result<(User, Mode, Score, Beatmap, BeatmapInfoWithPP)> {
    let osu_client = &env.client;
    let Some(u) = osu_client
        .user(&UserID::from_string(username), |f| f)
        .await?
    else {
        return Err(Error::msg("user not found"));
    };
    async fn find_score(client: &OsuHttpClient, u: &User) -> Result<Option<(Score, Mode)>> {
        for mode in &[
            u.preferred_mode,
            Mode::Std,
            Mode::Taiko,
            Mode::Catch,
            Mode::Mania,
        ] {
            let scores = client
                .user_best(UserID::ID(u.id), |f| f.mode(*mode))
                .await?
                .get_all()
                .await?;
            if let Some(v) = scores.into_iter().choose(&mut rand::thread_rng()) {
                return Ok(Some((v, *mode)));
            }
        }
        Ok(None)
    }
    let Some((score, mode)) = find_score(osu_client, &u).await? else {
        return Err(Error::msg(
            "No plays found in this account! Play something first...!",
        ));
    };
    let beatmap = osu_client
        .beatmaps(BeatmapRequestKind::Beatmap(score.beatmap_id), |f| {
            f.mode(mode, true)
        })
        .await?
        .into_iter()
        .next()
        .unwrap();
    let info = env
        .oppai
        .get_beatmap(beatmap.beatmap_id)
        .await?
        .get_possible_pp_with(mode, Mods::NOMOD);
    Ok((u, mode, score, beatmap, info))
}

const SAVE_BUTTON: &str = "youmubot::osu::save";
pub(crate) fn save_button() -> CreateActionRow {
    CreateActionRow::Buttons(vec![CreateButton::new(SAVE_BUTTON)
        .label("I'm done!")
        .emoji('👌')
        .style(serenity::all::ButtonStyle::Primary)])
}
pub(crate) async fn handle_save_respond(
    ctx: &Context,
    env: &OsuEnv,
    sender: serenity::all::UserId,
    reply: &mut impl CanEdit,
    beatmap: &Beatmap,
    user: crate::models::User,
    mode: Mode,
) -> Result<()> {
    let osu_client = &env.client;
    async fn check(client: &OsuHttpClient, u: &User, mode: Mode, map_id: u64) -> Result<bool> {
        Ok(client
            .user_recent(UserID::ID(u.id), |f| f.mode(mode))
            .await?
            .get(0)
            .await?
            .is_some_and(|s| s.beatmap_id == map_id))
    }
    let msg_id = reply.get_message().await?.id;
    let recv = InteractionCollector::create(&ctx, msg_id).await?;
    let timeout = std::time::Duration::from_secs(300) + beatmap.difficulty.total_length;
    let completed = loop {
        let Some(reaction) = recv.next(timeout).await else {
            break false;
        };
        if reaction == SAVE_BUTTON && check(osu_client, &user, mode, beatmap.beatmap_id).await? {
            break true;
        }
    };
    if !completed {
        reply
            .apply_edit(
                CreateReply::default()
                    .content(format!(
                        "Setting username to **{}** failed due to timeout. Please try again!",
                        user.username
                    ))
                    .components(vec![]),
            )
            .await?;
        return Ok(());
    }

    add_user(sender, &user, &env).await?;
    let ex = UserExtras::from_user(env, &user, mode).await?;
    reply
        .apply_edit(
            CreateReply::default()
                .content(
                    MessageBuilder::new()
                        .push("Youmu is now tracking user ")
                        .push(sender.mention().to_string())
                        .push(" with osu! account ")
                        .push(user.mention().to_string())
                        .build(),
                )
                .embed(user_embed(user.clone(), ex))
                .components(vec![]),
        )
        .await?;
    Ok(())
}

#[command]
#[description = "Save the given username as someone's username."]
#[owners_only]
#[usage = "[ping user]/[username or user_id]"]
#[delimiters(" ")]
#[num_args(2)]
pub async fn forcesave(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let osu_client = &env.client;

    let target = args.single::<UserId>()?.0;

    let username = args.quoted().trimmed().single::<String>()?;
    let Some(u) = osu_client
        .user(&UserID::from_string(username.clone()), |f| f)
        .await?
    else {
        msg.reply(&ctx, "user not found...").await?;
        return Ok(());
    };
    add_user(target, &u, &env).await?;
    let ex = UserExtras::from_user(&env, &u, u.preferred_mode).await?;
    msg.channel_id
        .send_message(
            &ctx,
            CreateMessage::new()
                .reference_message(msg)
                .content(
                    MessageBuilder::new()
                        .push("Youmu is now tracking user ")
                        .push(target.mention().to_string())
                        .push(" with osu! account ")
                        .push_bold_safe(username)
                        .build(),
                )
                .embed(user_embed(u, ex)),
        )
        .await?;
    Ok(())
}

async fn add_user(target: serenity::model::id::UserId, user: &User, env: &OsuEnv) -> Result<()> {
    let modes = [Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania]
        .into_iter()
        .map(|mode| {
            let mode = mode.clone();
            async move {
                let pp = async {
                    env.client
                        .user(&UserID::ID(user.id), |f| f.mode(mode))
                        .await
                        .pls_ok()
                        .unwrap_or(None)
                        .and_then(|u| u.pp)
                };
                let map_length_age = UserExtras::from_user(env, user, mode);
                let (pp, ex) = join!(pp, map_length_age);
                pp.zip(ex.ok()).map(|(pp, ex)| {
                    (
                        mode,
                        OsuUserMode {
                            pp,
                            map_length: ex.map_length,
                            map_age: ex.map_age,
                            last_update: Utc::now(),
                        },
                    )
                })
            }
        })
        .collect::<stream::FuturesOrdered<_>>()
        .filter_map(future::ready)
        .collect::<Map<_, _>>()
        .await;

    let u = OsuUser {
        user_id: target,
        username: user.username.clone().into(),
        preferred_mode: user.preferred_mode,
        id: user.id,
        failures: 0,
        modes,
    };
    env.saved_users.new_user(u).await?;
    Ok(())
}

/// Stores extra information to create an user embed.
pub(crate) struct UserExtras {
    pub map_length: f64,
    pub map_age: i64,
    pub best_score: Option<(Score, BeatmapWithMode, BeatmapInfo)>,
}

impl UserExtras {
    // Collect UserExtras from the given user.
    pub async fn from_user(env: &OsuEnv, user: &User, mode: Mode) -> Result<Self> {
        let scores = {
            match env
                .client
                .user_best(UserID::ID(user.id), |f| f.mode(mode))
                .await
                .pls_ok()
            {
                Some(v) => v.get_all().await.pls_ok().unwrap_or_else(Vec::new),
                None => Vec::new(),
            }
        };

        let (length, age) = join!(
            calculate_weighted_map_length(&scores, &env.beatmaps, mode),
            calculate_weighted_map_age(&scores, &env.beatmaps, mode)
        );
        let best = if let Some(s) = scores.into_iter().next() {
            let beatmap = env.beatmaps.get_beatmap(s.beatmap_id, mode).await?;
            let info = env
                .oppai
                .get_beatmap(s.beatmap_id)
                .await?
                .get_info_with(mode, &s.mods);
            Some((s, BeatmapWithMode(beatmap, Some(mode)), info))
        } else {
            None
        };

        Ok(Self {
            map_length: length.unwrap_or(0.0),
            map_age: age.unwrap_or(0),
            best_score: best,
        })
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Default)]
enum Nth {
    #[default]
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
            let v = s.split_at("#".len()).1.parse::<u8>()?;
            if v > 0 {
                Ok(Nth::Nth(v - 1))
            } else {
                Err(Error::msg("number has to be at least 1"))
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ListingArgs {
    pub nth: Nth,
    pub style: ScoreListStyle,
    pub mode: Mode,
    pub user: UserHeader,
}

impl ListingArgs {
    pub async fn from_params(
        env: &OsuEnv,
        index: Option<u8>,
        style: ScoreListStyle,
        mode_override: Option<Mode>,
        user: Option<UsernameArg>,
        sender: serenity::all::UserId,
    ) -> Result<Self> {
        let nth = index
            .filter(|&v| 1 <= v && v <= MAX_TOP_SCORES_INDEX as u8)
            .map(|v| v - 1)
            .map(Nth::Nth)
            .unwrap_or_default();
        let (mode, user) = user_header_or_default_id(user, env, sender).await?;
        let mode = mode_override.unwrap_or(mode);
        Ok(Self {
            nth,
            style,
            mode,
            user,
        })
    }
    pub async fn parse(
        env: &OsuEnv,
        msg: &Message,
        args: &mut Args,
        default_style: ScoreListStyle,
    ) -> Result<ListingArgs> {
        let nth = args.single::<Nth>().unwrap_or(Nth::All);
        let style = args.single::<ScoreListStyle>().unwrap_or(default_style);
        let mode_override = args.single::<ModeArg>().map(|v| v.0).ok();
        let (mode, user) =
            user_header_from_args(args.single::<UsernameArg>().ok(), env, msg).await?;
        let mode = mode_override.unwrap_or(mode);
        Ok(Self {
            nth,
            style,
            mode,
            user,
        })
    }
}

async fn user_header_or_default_id(
    arg: Option<UsernameArg>,
    env: &OsuEnv,
    default_user: serenity::all::UserId,
) -> Result<(Mode, UserHeader)> {
    let (mode, user) = match arg {
        Some(UsernameArg::Raw(r)) => {
            let user = env
                .client
                .user(&UserID::Username(Arc::new(r)), |f| f)
                .await?
                .ok_or(Error::msg("User not found"))?;
            (user.preferred_mode, user.into())
        }
        Some(UsernameArg::Tagged(t)) => {
            let user = env.saved_users.by_user_id(t).await?.ok_or_else(|| {
                Error::msg(format!("{} does not have a saved account!", t.mention()))
            })?;
            (user.preferred_mode, user.into())
        }
        None => {
            let user = env.saved_users.by_user_id(default_user).await?
                        .ok_or(Error::msg("You do not have a saved account! Use `osu save` command to save your osu! account."))?;
            (user.preferred_mode, user.into())
        }
    };
    Ok((mode, user))
}

async fn user_header_from_args(
    arg: Option<UsernameArg>,
    env: &OsuEnv,
    msg: &Message,
) -> Result<(Mode, UserHeader)> {
    user_header_or_default_id(arg, env, msg.author.id).await
}

#[command]
#[aliases("rs", "rc", "r")]
#[description = "Gets an user's recent play"]
#[usage = "#[the nth recent play = --all] / [style (table or grid) = --table] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[delimiters("/", " ")]
#[max_args(4)]
pub async fn recent(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = ListingArgs::parse(&env, msg, &mut args, ScoreListStyle::Table).await?;

    let osu_client = &env.client;
    let mut plays = osu_client
        .user_recent(UserID::ID(user.id), |f| f.mode(mode))
        .await?;
    match nth {
        Nth::All => {
            let header = format!("Here are the recent plays by {}!", user.mention());
            let reply = msg.reply(ctx, &header).await?;
            style
                .display_scores(plays, ctx, reply.guild_id, (reply, ctx).with_header(header))
                .await?;
        }
        Nth::Nth(nth) => {
            let play = plays
                .get(nth as usize)
                .await?
                .ok_or(Error::msg("No such play"))?
                .clone();
            let attempts = {
                let mut count = 0usize;
                while plays
                    .get(nth as usize + count + 1)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|p| {
                        p.beatmap_id == play.beatmap_id
                            && p.mode == play.mode
                            && p.mods == play.mods
                    })
                {
                    count += 1;
                }
                count
            };
            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, Some(mode));

            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content(format!(
                            "Here is the #{} recent play by {}!",
                            nth + 1,
                            user.mention()
                        ))
                        .embed(
                            score_embed(&play, &beatmap_mode, &content, user)
                                .footer(format!("Attempt #{}", attempts))
                                .build(),
                        )
                        .components(vec![score_components(msg.guild_id)])
                        .reference_message(msg),
                )
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&env, msg.channel_id, &beatmap_mode).await?;
        }
    }
    Ok(())
}

#[command]
#[aliases("pin")]
#[description = "Gets an user's pinned plays"]
#[usage = "#[the nth recent play = --all] / [style (table or grid) = --table] / [mode (std, taiko, mania, catch) = std] / [username / user id = your saved id]"]
#[example = "#1 / taiko / natsukagami"]
#[delimiters("/", " ")]
#[max_args(4)]
pub async fn pins(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = ListingArgs::parse(&env, msg, &mut args, ScoreListStyle::Grid).await?;

    let osu_client = &env.client;

    let mut plays = osu_client
        .user_pins(UserID::ID(user.id), |f| f.mode(mode))
        .await?;
    match nth {
        Nth::All => {
            let header = format!("Here are the pinned plays by `{}`!", user.username);
            let reply = msg.reply(ctx, &header).await?;
            style
                .display_scores(plays, ctx, reply.guild_id, (reply, ctx).with_header(header))
                .await?;
        }
        Nth::Nth(nth) => {
            let play = plays
                .get(nth as usize)
                .await?
                .ok_or(Error::msg("No such play"))?;
            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, Some(mode));

            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content("Here is the play that you requested".to_string())
                        .embed(score_embed(&play, &beatmap_mode, &content, user).build())
                        .components(vec![score_components(msg.guild_id)])
                        .reference_message(msg),
                )
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&env, msg.channel_id, &beatmap_mode).await?;
        }
    }
    Ok(())
}

/// Get beatmapset.
struct OptBeatmapSet;

impl FromStr for OptBeatmapSet {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--set" | "-s" | "--beatmapset" => Ok(Self),
            _ => Err(Error::msg("not opt beatmapset")),
        }
    }
}

pub(crate) async fn load_beatmap_from_channel(
    env: &OsuEnv,
    channel_id: serenity::all::ChannelId,
) -> Option<EmbedType> {
    let BeatmapWithMode(b, m) = cache::get_beatmap(env, channel_id).await.ok().flatten()?;
    let mods = Mods::NOMOD.clone();
    let info = env
        .oppai
        .get_beatmap(b.beatmap_id)
        .await
        .pls_ok()?
        .get_possible_pp_with(m.unwrap_or(b.mode), &mods);
    Some(EmbedType::Beatmap(
        Box::new(b),
        m,
        info,
        Mods::NOMOD.clone(),
    ))
}

#[derive(PartialEq, Eq, Clone, Copy, Default)]
pub(crate) enum LoadRequest {
    #[default]
    Any,
    Beatmap,
    Beatmapset,
}

/// Load the mentioned beatmap from the given message.
pub(crate) async fn load_beatmap(
    env: &OsuEnv,
    channel_id: serenity::all::ChannelId,
    referenced: Option<&impl Borrow<Message>>,
    req: LoadRequest,
) -> Option<EmbedType> {
    /* If the request is Beatmapset, we keep a fallback match on beatmap, and later convert it to a beatmapset. */
    let mut fallback: Option<EmbedType> = None;
    async fn collect_referenced(
        env: &OsuEnv,
        fallback: &mut Option<EmbedType>,
        req: LoadRequest,
        replied: &impl Borrow<Message>,
    ) -> Option<EmbedType> {
        use link_parser::*;
        async fn try_content(
            env: &OsuEnv,
            req: LoadRequest,
            fallback: &mut Option<EmbedType>,
            content: &str,
        ) -> Option<EmbedType> {
            parse_short_links(env, content)
                .filter(|e| {
                    future::ready(match &e.embed {
                        EmbedType::Beatmap(_, _, _, _) => {
                            if fallback.is_none() {
                                fallback.replace(e.embed.clone());
                            }
                            req == LoadRequest::Beatmap || req == LoadRequest::Any
                        }
                        EmbedType::Beatmapset(_, _) => {
                            req == LoadRequest::Beatmapset || req == LoadRequest::Any
                        }
                    })
                })
                .next()
                .await
                .map(|v| v.embed)
        }
        if let Some(v) = try_content(env, req, fallback, &replied.borrow().content).await {
            return Some(v);
        }
        for embed in &replied.borrow().embeds {
            for field in &embed.fields {
                if let Some(v) = try_content(env, req, fallback, &field.value).await {
                    return Some(v);
                }
            }
            if let Some(desc) = &embed.description {
                if let Some(v) = try_content(env, req, fallback, desc).await {
                    return Some(v);
                }
            }
        }
        None
    }

    let embed = match referenced {
        Some(r) => collect_referenced(env, &mut fallback, req, r).await,
        None => load_beatmap_from_channel(env, channel_id).await,
    };

    if req == LoadRequest::Beatmapset {
        if embed.is_none() {
            if let Some(EmbedType::Beatmap(b, mode, _, _)) = fallback {
                return EmbedType::from_beatmapset_id(env, b.beatmapset_id, mode)
                    .await
                    .ok();
            }
        }
    }
    embed
}

#[command]
#[aliases("map")]
#[description = "Show information from the last queried beatmap."]
#[usage = "[--set/-s/--beatmapset] / [mods = no mod]"]
#[delimiters(" ")]
#[max_args(2)]
pub async fn last(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let beatmapset = args.find::<OptBeatmapSet>().is_ok();
    let Some(embed) = load_beatmap(
        &env,
        msg.channel_id,
        msg.referenced_message.as_ref(),
        if beatmapset {
            LoadRequest::Beatmapset
        } else {
            LoadRequest::Any
        },
    )
    .await
    else {
        msg.reply(&ctx, "No beatmap was queried on this channel.")
            .await?;
        return Ok(());
    };
    let umods = args.find::<UnparsedMods>().ok();

    let content_type = embed.mention();
    match embed {
        EmbedType::Beatmap(b, mode_, _, mods) => {
            let mode = mode_.unwrap_or(b.mode);
            let mods = match umods {
                Some(m) => m.to_mods(mode)?,
                None => mods,
            };
            let info = env
                .oppai
                .get_beatmap(b.beatmap_id)
                .await?
                .get_possible_pp_with(mode, &mods);
            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content(format!("Information for {}", content_type))
                        .embed(beatmap_embed(&b, mode, &mods, &info))
                        .components(vec![beatmap_components(mode, msg.guild_id)])
                        .reference_message(msg),
                )
                .await?;
            // Save the beatmap...
            cache::save_beatmap(&env, msg.channel_id, &BeatmapWithMode(*b, mode_)).await?;
        }
        EmbedType::Beatmapset(beatmaps, mode) => {
            let reply = msg
                .reply(&ctx, format!("Information for {}", content_type))
                .await?;
            display::display_beatmapset(ctx, beatmaps, mode, umods, msg.guild_id, (reply, ctx))
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
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let Some(embed) = load_beatmap(
        &env,
        msg.channel_id,
        msg.referenced_message.as_ref(),
        LoadRequest::Any,
    )
    .await
    else {
        msg.reply(&ctx, "No beatmap queried on this channel.")
            .await?;
        return Ok(());
    };

    let umods = args.find::<UnparsedMods>().ok();
    let style = args
        .single::<ScoreListStyle>()
        .unwrap_or(ScoreListStyle::Grid);
    let username_arg = args.single::<UsernameArg>().ok();
    let (_, user) = user_header_from_args(username_arg, &env, msg).await?;

    let scores = do_check(&env, &embed, umods, &user).await?;

    if scores.is_empty() {
        msg.reply(&ctx, "No scores found").await?;
        return Ok(());
    }
    let header = format!(
        "Here are the scores by `{}` on {}!",
        &user.username,
        embed.mention()
    );
    let reply = msg.reply(&ctx, &header).await?;
    style
        .display_scores(scores, ctx, msg.guild_id, (reply, ctx).with_header(header))
        .await?;

    Ok(())
}

pub(crate) async fn do_check(
    env: &OsuEnv,
    embed: &EmbedType,
    mods: Option<UnparsedMods>,
    user: &UserHeader,
) -> Result<Vec<Score>> {
    async fn fetch_for_beatmap(
        env: &OsuEnv,
        b: &Beatmap,
        mode_override: Option<Mode>,
        mods: &Option<UnparsedMods>,
        user: &UserHeader,
    ) -> Result<Vec<Score>> {
        let osu_client = &env.client;
        let m = mode_override.unwrap_or(b.mode);
        let mods = mods.clone().and_then(|t| t.to_mods(m).ok());
        osu_client
            .scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(m))
            .and_then(|v| v.get_all())
            .map_ok(move |mut v| {
                v.retain(|s| mods.as_ref().is_none_or(|m| s.mods.contains(&m)));
                v
            })
            .await
    }

    let mut scores = match embed {
        EmbedType::Beatmap(beatmap, mode, _, _) => {
            fetch_for_beatmap(env, &**beatmap, *mode, &mods, user).await?
        }
        EmbedType::Beatmapset(vec, mode) => vec
            .iter()
            .map(|b| fetch_for_beatmap(env, b, *mode, &mods, user))
            .collect::<FuturesUnordered<_>>()
            .try_collect::<Vec<_>>()
            .await?
            .concat(),
    };
    scores.sort_by(|a, b| {
        b.pp.unwrap_or(-1.0)
            .partial_cmp(&a.pp.unwrap_or(-1.0))
            .unwrap()
    });
    Ok(scores)
}

#[command]
#[aliases("t")]
#[description = "Get the n-th top record of an user."]
#[usage = "#[n-th = --all] / [style (table or grid) = --table] / [mode (std, taiko, catch, mania)] = std / [username or user_id = your saved user id]"]
#[example = "#2 / taiko / natsukagami"]
#[max_args(4)]
pub async fn top(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = ListingArgs::parse(&env, msg, &mut args, ScoreListStyle::default()).await?;
    let osu_client = &env.client;

    let mut plays = osu_client
        .user_best(UserID::ID(user.id), |f| f.mode(mode))
        .await?;

    match nth {
        Nth::Nth(nth) => {
            let play = plays
                .get(nth as usize)
                .await?
                .ok_or(Error::msg("No such play"))?;

            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, Some(mode));

            msg.channel_id
                .send_message(&ctx, {
                    CreateMessage::new()
                        .content(format!(
                            "Here is the #{} top play by {}!",
                            nth + 1,
                            user.mention()
                        ))
                        .embed(
                            score_embed(&play, &beatmap, &content, user)
                                .top_record(nth + 1)
                                .build(),
                        )
                        .components(vec![score_components(msg.guild_id)])
                })
                .await?;

            // Save the beatmap...
            cache::save_beatmap(&env, msg.channel_id, &beatmap).await?;
        }
        Nth::All => {
            let header = format!("Here are the top plays by {}!", user.mention());
            let reply = msg.reply(&ctx, &header).await?;
            style
                .display_scores(plays, ctx, msg.guild_id, (reply, ctx).with_header(header))
                .await?;
        }
    }
    Ok(())
}

#[command("cleancache")]
#[owners_only]
#[description = "Clean the beatmap cache."]
#[usage = "[--oppai to clear oppai cache as well]"]
#[max_args(1)]
pub async fn clean_cache(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    env.beatmaps.clear().await?;

    if args.remains() == Some("--oppai") {
        env.oppai.clear().await?;
    }
    msg.reply_ping(ctx, "Beatmap cache cleared!").await?;
    Ok(())
}

async fn get_user(
    ctx: &Context,
    env: &OsuEnv,
    msg: &Message,
    mut args: Args,
    mode_override: impl Into<Option<Mode>>,
) -> CommandResult {
    let (mode, user) = user_header_from_args(args.single::<UsernameArg>().ok(), env, msg).await?;
    let mode = mode_override.into().unwrap_or(mode);
    let user = env
        .client
        .user(&UserID::ID(user.id), |f| f.mode(mode))
        .await?;

    match user {
        Some(u) => {
            let ex = UserExtras::from_user(env, &u, mode).await?;
            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content(format!("Here is {}'s **{}** profile!", u.mention(), mode))
                        .embed(user_embed(u, ex)),
                )
                .await?;
        }
        None => {
            msg.reply(&ctx, "🔍 user not found!").await?;
        }
    };
    Ok(())
}

const SCALING_FACTOR: f64 = 0.975;
static SCALES: std::sync::OnceLock<Box<[f64]>> = std::sync::OnceLock::new();
fn scales() -> &'static [f64] {
    SCALES.get_or_init(|| {
        (0..256)
            .map(|r| SCALING_FACTOR.powi(r))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    })
}

pub(in crate::discord) async fn calculate_weighted_map_length(
    from_scores: impl IntoIterator<Item = &Score>,
    cache: &BeatmapMetaCache,
    mode: Mode,
) -> Result<f64> {
    let scores = from_scores
        .into_iter()
        .map(|s| async move {
            let beatmap = cache.get_beatmap(s.beatmap_id, mode).await?;
            Ok(beatmap
                .difficulty
                .apply_mods(&s.mods, 0.0 /* dont care */)
                .drain_length
                .as_secs_f64()) as Result<_>
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?;
    Ok(scores.into_iter().zip(scales()).map(|(a, b)| a * b).sum())
}

pub(in crate::discord) async fn calculate_weighted_map_age(
    from_scores: impl IntoIterator<Item = &Score>,
    cache: &BeatmapMetaCache,
    mode: Mode,
) -> Result<i64> {
    let scores = from_scores
        .into_iter()
        .map(|s| async move {
            let beatmap = cache.get_beatmap(s.beatmap_id, mode).await?;
            Ok(
                if let crate::ApprovalStatus::Ranked(at) = beatmap.approval {
                    at.timestamp() as f64
                } else {
                    0.0
                },
            ) as Result<_>
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?;
    Ok((scores
        .iter()
        .zip(scales().iter())
        .map(|(a, b)| a * b)
        .sum::<f64>()
        / scales().iter().take(scores.len()).sum::<f64>())
    .floor() as i64)
}

pub(crate) fn time_before_now(time: &chrono::DateTime<Utc>) -> String {
    let dur = Utc::now() - time;
    if dur.num_days() >= 365 {
        format!("{}Y", dur.num_days() / 365)
    } else if dur.num_days() >= 30 {
        format!("{}M", dur.num_days() / 30)
    } else if dur.num_days() >= 1 {
        format!("{}d", dur.num_days())
    } else if dur.num_hours() >= 1 {
        format!("{}h", dur.num_hours())
    } else if dur.num_minutes() >= 1 {
        format!("{}m", dur.num_minutes())
    } else {
        format!("{}s", dur.num_seconds())
    }
}

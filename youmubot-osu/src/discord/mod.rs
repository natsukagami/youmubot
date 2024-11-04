use std::{borrow::Borrow, collections::HashMap as Map, str::FromStr, sync::Arc};

use chrono::Utc;
use futures_util::join;
use interaction::{beatmap_components, score_components};
use rand::seq::IteratorRandom;
use serenity::{
    builder::{CreateMessage, EditMessage},
    collector,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
};

use db::{OsuLastBeatmap, OsuSavedUsers, OsuUser, OsuUserMode};
use embeds::{beatmap_embed, score_embed, user_embed};
pub use hook::{dot_osu_hook, hook, score_hook};
use server_rank::{SERVER_RANK_COMMAND, SHOW_LEADERBOARD_COMMAND};
use stream::FuturesOrdered;
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
    OsuClient as OsuHttpClient, UserHeader,
};

mod announcer;
pub(crate) mod beatmap_cache;
mod cache;
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
pub(crate) struct BeatmapWithMode(pub Beatmap, pub Mode);

impl BeatmapWithMode {
    pub fn short_link(&self, mods: &Mods) -> String {
        self.0.short_link(Some(self.1), mods)
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
pub async fn save(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let osu_client = &env.client;

    let user = args.single::<String>()?;
    let u = match osu_client.user(&UserID::from_string(user), |f| f).await? {
        Some(u) => u,
        None => {
            msg.reply(&ctx, "user not found...").await?;
            return Ok(());
        }
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
                .await?;
            if let Some(v) = scores.into_iter().choose(&mut rand::thread_rng()) {
                return Ok(Some((v, *mode)));
            }
        }
        Ok(None)
    }
    let (score, mode) = match find_score(osu_client, &u).await? {
        Some(v) => v,
        None => {
            msg.reply(
                &ctx,
                "No plays found in this account! Play something first...!",
            )
            .await?;
            return Ok(());
        }
    };

    async fn check(client: &OsuHttpClient, u: &User, map_id: u64) -> Result<bool> {
        Ok(client
            .user_recent(UserID::ID(u.id), |f| f.mode(Mode::Std).limit(1))
            .await?
            .into_iter()
            .take(1)
            .any(|s| s.beatmap_id == map_id))
    }

    let reply = msg.reply(
        &ctx,
        format!(
            "To set your osu username to **{}**, please make your most recent play \
            be the following map: `/b/{}` in **{}** mode! \
        It does **not** have to be a pass, and **NF** can be used! \
        React to this message with 👌 within 5 minutes when you're done!",
            u.username,
            score.beatmap_id,
            mode.as_str_new_site()
        ),
    );
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
        .get_possible_pp_with(mode, Mods::NOMOD)?;
    let mut reply = reply.await?;
    reply
        .edit(
            &ctx,
            EditMessage::new()
                .embed(beatmap_embed(&beatmap, mode, Mods::NOMOD, info))
                .components(vec![beatmap_components(msg.guild_id)]),
        )
        .await?;
    let reaction = reply.react(&ctx, '👌').await?;
    let completed = loop {
        let emoji = reaction.emoji.clone();
        let user_reaction = collector::ReactionCollector::new(ctx)
            .message_id(reply.id)
            .author_id(msg.author.id)
            .filter(move |r| r.emoji == emoji)
            .timeout(std::time::Duration::from_secs(300) + beatmap.difficulty.total_length)
            .next()
            .await;
        if let Some(ur) = user_reaction {
            if check(osu_client, &u, score.beatmap_id).await? {
                break true;
            }
            ur.delete(&ctx).await?;
        } else {
            break false;
        }
    };
    if !completed {
        reply
            .edit(
                &ctx,
                EditMessage::new()
                    .content(format!(
                        "Setting username to **{}** failed due to timeout. Please try again!",
                        u.username
                    ))
                    .embeds(vec![])
                    .components(vec![]),
            )
            .await?;
        reaction.delete(&ctx).await?;
        return Ok(());
    }

    let username = u.username.clone();
    add_user(msg.author.id, &u, &env).await?;
    let ex = UserExtras::from_user(&env, &u, mode).await?;
    msg.channel_id
        .send_message(
            &ctx,
            CreateMessage::new()
                .reference_message(msg)
                .content(
                    MessageBuilder::new()
                        .push("Youmu is now tracking user ")
                        .push(msg.author.mention().to_string())
                        .push(" with osu! account ")
                        .push_bold_safe(username)
                        .build(),
                )
                .add_embed(user_embed(u, ex)),
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
    let user: Option<User> = osu_client
        .user(&UserID::from_string(username.clone()), |f| f)
        .await?;
    match user {
        Some(u) => {
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
        }
        None => {
            msg.reply(&ctx, "user not found...").await?;
        }
    }
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
        let scores = env
            .client
            .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))
            .await
            .pls_ok()
            .unwrap_or_else(std::vec::Vec::new);

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
                .get_info_with(mode, &s.mods)?;
            Some((s, BeatmapWithMode(beatmap, mode), info))
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

async fn user_header_from_args(
    arg: Option<UsernameArg>,
    env: &OsuEnv,
    msg: &Message,
) -> Result<(Mode, UserHeader)> {
    let (mode, user) = match arg {
        Some(UsernameArg::Raw(r)) => {
            let user = env
                .client
                .user(&UserID::Username(r), |f| f)
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
            let user = env.saved_users.by_user_id(msg.author.id).await?
                        .ok_or(Error::msg("You do not have a saved account! Use `osu save` command to save your osu! account."))?;
            (user.preferred_mode, user.into())
        }
    };
    Ok((mode, user))
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
    let plays = osu_client
        .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(50))
        .await?;
    match nth {
        Nth::All => {
            let reply = msg
                .reply(
                    ctx,
                    format!("Here are the recent plays by `{}`!", user.username),
                )
                .await?;
            style
                .display_scores(plays, mode, ctx, reply.guild_id, reply)
                .await?;
        }
        Nth::Nth(nth) => {
            let Some(play) = plays.get(nth as usize) else {
                Err(Error::msg("No such play"))?
            };
            let attempts = plays
                .iter()
                .skip(nth as usize)
                .take_while(|p| p.beatmap_id == play.beatmap_id && p.mods == play.mods)
                .count();
            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content("Here is the play that you requested".to_string())
                        .embed(
                            score_embed(play, &beatmap_mode, &content, user)
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

    let plays = osu_client
        .user_pins(UserID::ID(user.id), |f| f.mode(mode).limit(50))
        .await?;
    match nth {
        Nth::All => {
            let reply = msg
                .reply(
                    ctx,
                    format!("Here are the pinned plays by `{}`!", user.username),
                )
                .await?;
            style
                .display_scores(plays, mode, ctx, reply.guild_id, reply)
                .await?;
        }
        Nth::Nth(nth) => {
            let Some(play) = plays.get(nth as usize) else {
                Err(Error::msg("No such play"))?
            };
            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap_mode = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content("Here is the play that you requested".to_string())
                        .embed(score_embed(play, &beatmap_mode, &content, user).build())
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

/// Load the mentioned beatmap from the given message.
pub(crate) async fn load_beatmap(
    env: &OsuEnv,
    channel_id: serenity::all::ChannelId,
    referenced: Option<&impl Borrow<Message>>,
) -> Option<(BeatmapWithMode, Option<Mods>)> {
    use link_parser::{parse_short_links, EmbedType};
    if let Some(replied) = referenced {
        async fn try_content(
            env: &OsuEnv,
            content: &str,
        ) -> Option<(BeatmapWithMode, Option<Mods>)> {
            let tp = parse_short_links(env, content).next().await?;
            match tp.embed {
                EmbedType::Beatmap(b, _, mods) => {
                    let mode = tp.mode.unwrap_or(b.mode);
                    Some((BeatmapWithMode(*b, mode), Some(mods)))
                }
                _ => None,
            }
        }
        for embed in &replied.borrow().embeds {
            for field in &embed.fields {
                if let Some(v) = try_content(env, &field.value).await {
                    return Some(v);
                }
            }
            if let Some(desc) = &embed.description {
                if let Some(v) = try_content(env, desc).await {
                    return Some(v);
                }
            }
        }
        if let Some(v) = try_content(env, &replied.borrow().content).await {
            return Some(v);
        }
    }

    let b = cache::get_beatmap(env, channel_id).await.ok().flatten();
    b.map(|b| (b, None))
}

#[command]
#[aliases("map")]
#[description = "Show information from the last queried beatmap."]
#[usage = "[--set/-s/--beatmapset] / [mods = no mod]"]
#[delimiters(" ")]
#[max_args(2)]
pub async fn last(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let b = load_beatmap(&env, msg.channel_id, msg.referenced_message.as_ref()).await;
    let beatmapset = args.find::<OptBeatmapSet>().is_ok();

    match b {
        Some((bm, mods_def)) => {
            let mods = match args.find::<UnparsedMods>().ok() {
                Some(m) => m.to_mods(bm.mode())?,
                None => mods_def.unwrap_or_default(),
            };
            if beatmapset {
                let beatmapset = env.beatmaps.get_beatmapset(bm.0.beatmapset_id).await?;
                let reply = msg
                    .reply(&ctx, "Here is the beatmapset you requested!")
                    .await?;
                display::display_beatmapset(ctx.clone(), beatmapset, None, mods, reply).await?;
                return Ok(());
            }
            let info = env
                .oppai
                .get_beatmap(bm.0.beatmap_id)
                .await?
                .get_possible_pp_with(bm.1, &mods)?;
            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .content("Here is the beatmap you requested!")
                        .embed(beatmap_embed(&bm.0, bm.1, &mods, info))
                        .components(vec![beatmap_components(msg.guild_id)])
                        .reference_message(msg),
                )
                .await?;
            // Save the beatmap...
            cache::save_beatmap(&env, msg.channel_id, &bm).await?;
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
    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
    let bm = load_beatmap(&env, msg.channel_id, msg.referenced_message.as_ref()).await;

    let bm = match bm {
        Some((bm, _)) => bm,
        None => {
            msg.reply(&ctx, "No beatmap queried on this channel.")
                .await?;
            return Ok(());
        }
    };
    let mode = bm.1;
    let mods = args
        .find::<UnparsedMods>()
        .ok()
        .unwrap_or_default()
        .to_mods(mode)?;
    let style = args
        .single::<ScoreListStyle>()
        .unwrap_or(ScoreListStyle::Grid);
    let username_arg = args.single::<UsernameArg>().ok();
    let (_, user) = user_header_from_args(username_arg, &env, msg).await?;

    let scores = do_check(&env, &bm, &mods, &user).await?;

    if scores.is_empty() {
        msg.reply(&ctx, "No scores found").await?;
        return Ok(());
    }
    let reply = msg
        .reply(
            &ctx,
            format!(
                "Here are the scores by `{}` on `{}`!",
                &user.username,
                bm.short_link(&mods)
            ),
        )
        .await?;
    style
        .display_scores(scores, mode, ctx, msg.guild_id, reply)
        .await?;

    Ok(())
}

pub(crate) async fn do_check(
    env: &OsuEnv,
    bm: &BeatmapWithMode,
    mods: &Mods,
    user: &UserHeader,
) -> Result<Vec<Score>> {
    let BeatmapWithMode(b, m) = bm;

    let osu_client = &env.client;

    let mut scores = osu_client
        .scores(b.beatmap_id, |f| f.user(UserID::ID(user.id)).mode(*m))
        .await?
        .into_iter()
        .filter(|s| s.mods.contains(mods))
        .collect::<Vec<_>>();
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

    let plays = osu_client
        .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))
        .await?;

    match nth {
        Nth::Nth(nth) => {
            let Some(play) = plays.get(nth as usize) else {
                Err(Error::msg("no such play"))?
            };

            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, mode);

            msg.channel_id
                .send_message(&ctx, {
                    CreateMessage::new()
                        .content(format!(
                            "{}: here is the play that you requested",
                            msg.author
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
            let reply = msg
                .reply(
                    &ctx,
                    format!("Here are the top plays by `{}`!", user.username),
                )
                .await?;
            style
                .display_scores(plays, mode, ctx, msg.guild_id, reply)
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
                        .content(format!(
                            "{}: here is the user that you requested",
                            msg.author
                        ))
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
            // .scan(1.0, |a, _| {
            //     let old = *a;
            //     *a *= SCALING_FACTOR;
            //     Some(old)
            // })
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

use std::cmp::Ordering;

use super::*;
use cache::save_beatmap;
use display::display_beatmapset;
use embeds::ScoreEmbedBuilder;
use link_parser::EmbedType;
use poise::{ChoiceParameter, CreateReply};
use serenity::all::{CreateAttachment, User};
use server_rank::get_leaderboard_from_embed;

/// osu!-related command group.
#[poise::command(
    slash_command,
    subcommands(
        "profile",
        "top",
        "recent",
        "pinned",
        "save",
        "forcesave",
        "beatmap",
        "check",
        "ranks",
        "leaderboard",
        "clear_cache"
    ),
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn osu<U: HasOsuEnv>(_ctx: CmdContext<'_, U>) -> Result<()> {
    Ok(())
}

/// Returns top plays for a given player.
///
/// If no osu! username is given, defaults to the currently registered user.
#[poise::command(slash_command)]
async fn top<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Index of the score"]
    #[min = 1]
    #[max = 200] // SCORE_COUNT_LIMIT
    index: Option<u8>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
    #[description = "Game mode"] mode: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let username_arg = arg_from_username_or_discord(username, discord_name);
    let args = ListingArgs::from_params(
        env,
        index,
        style.unwrap_or(ScoreListStyle::Table),
        mode,
        username_arg,
        ctx.author().id,
    )
    .await?;

    ctx.defer().await?;
    let osu_client = &env.client;
    let mode = args.mode;
    let plays = osu_client
        .user_best(UserID::ID(args.user.id), |f| f.mode(mode))
        .await?;

    handle_listing(ctx, plays, args, |nth, b| b.top_record(nth), "top").await
}

/// Get an user's profile.
#[poise::command(slash_command)]
async fn profile<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Game mode"]
    #[rename = "mode"]
    mode_override: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let username_arg = arg_from_username_or_discord(username, discord_name);
    let (mode, user) = user_header_or_default_id(username_arg, env, ctx.author().id).await?;
    let mode = mode_override.unwrap_or(mode);

    ctx.defer().await?;

    let user = env
        .client
        .user(&UserID::ID(user.id), |f| f.mode(mode))
        .await?;

    match user {
        Some(u) => {
            let ex = UserExtras::from_user(env, &u, mode).await?;
            ctx.send(
                CreateReply::default()
                    .content(format!("Here is {}'s **{}** profile!", u.mention(), mode))
                    .embed(user_embed(u, ex)),
            )
            .await?;
        }
        None => {
            ctx.reply("🔍 user not found!").await?;
        }
    };
    Ok(())
}

/// Returns recent plays from a given player.
///
/// If no osu! username is given, defaults to the currently registered user.
#[poise::command(slash_command)]
async fn recent<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Index of the score"]
    #[min = 1]
    #[max = 50]
    index: Option<u8>,
    #[description = "Only include passed scores"] passes_only: Option<bool>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
    #[description = "Game mode"] mode: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let args = arg_from_username_or_discord(username, discord_name);
    let style = style.unwrap_or(ScoreListStyle::Table);
    let include_fails = !passes_only.unwrap_or(false);

    let args = ListingArgs::from_params(env, index, style, mode, args, ctx.author().id).await?;

    ctx.defer().await?;

    let osu_client = &env.client;
    let mode = args.mode;
    let plays = osu_client
        .user_recent(UserID::ID(args.user.id), |f| {
            f.mode(mode).include_fails(include_fails)
        })
        .await?;

    handle_listing(ctx, plays, args, |_, b| b, "recent").await
}

/// Returns pinned plays from a given player.
///
/// If no osu! username is given, defaults to the currently registered user.
#[poise::command(slash_command)]
async fn pinned<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Index of the score"]
    #[min = 1]
    #[max = 50]
    index: Option<u8>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
    #[description = "Game mode"] mode: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let args = arg_from_username_or_discord(username, discord_name);
    let style = style.unwrap_or(ScoreListStyle::Table);

    let args = ListingArgs::from_params(env, index, style, mode, args, ctx.author().id).await?;

    ctx.defer().await?;

    let osu_client = &env.client;
    let mode = args.mode;
    let plays = osu_client
        .user_pins(UserID::ID(args.user.id), |f| f.mode(mode))
        .await?;

    handle_listing(ctx, plays, args, |_, b| b, "pinned").await
}

/// Save your osu! profile into Youmu's database for tracking and quick commands.
#[poise::command(slash_command)]
pub async fn save<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "The osu! username to set to"] username: String,
) -> Result<()> {
    let env = ctx.data().osu_env();
    ctx.defer().await?;
    let (u, mode, score, beatmap, info) = find_save_requirements(env, username).await?;
    let reply = ctx
        .send(
            CreateReply::default()
                .content(save_request_message(&u.username, score.beatmap_id, mode))
                .embed(beatmap_embed(&beatmap, mode, Mods::NOMOD, &info))
                .components(vec![
                    beatmap_components(mode, ctx.guild_id()),
                    save_button(),
                ]),
        )
        .await?;
    let mut p = (reply, ctx);
    match handle_save_respond(
        ctx.serenity_context(),
        env,
        ctx.author().id,
        &mut p,
        &beatmap,
        u,
        mode,
    )
    .await
    {
        Ok(_) => (),
        Err(e) => {
            p.0.delete(ctx).await?;
            return Err(e);
        }
    };

    Ok(())
}

/// Force-save an osu! profile into Youmu's database for tracking and quick commands.
#[poise::command(slash_command, owners_only)]
pub async fn forcesave<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "The osu! username to set to"] username: String,
    #[description = "The discord user to assign to"] discord_name: User,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let osu_client = &env.client;
    ctx.defer().await?;
    let Some(u) = osu_client
        .user(&UserID::from_string(username.clone()), |f| f)
        .await?
    else {
        return Err(Error::msg("osu! user not found"));
    };
    add_user(discord_name.id, &u, env).await?;
    let ex = UserExtras::from_user(env, &u, u.preferred_mode).await?;
    ctx.send(
        CreateReply::default()
            .content(
                MessageBuilder::new()
                    .push("Youmu is now tracking user ")
                    .push(discord_name.mention().to_string())
                    .push(" with osu! account ")
                    .push_bold_safe(username)
                    .build(),
            )
            .embed(user_embed(u, ex)),
    )
    .await?;
    Ok(())
}

async fn handle_listing<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    mut plays: impl Scores,
    listing_args: ListingArgs,
    transform: impl for<'a> Fn(u8, ScoreEmbedBuilder<'a>) -> ScoreEmbedBuilder<'a>,
    listing_kind: &'static str,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = listing_args;

    match nth {
        Nth::Nth(nth) => {
            let play = if let Some(play) = plays.get(nth as usize).await? {
                play
            } else {
                return Err(Error::msg("no such play"))?;
            };

            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, Some(mode));

            ctx.send({
                CreateReply::default()
                    .content(format!(
                        "Here is the #{} {} play by {}!",
                        nth + 1,
                        listing_kind,
                        user.mention()
                    ))
                    .embed({
                        let mut b = transform(nth + 1, score_embed(play, &beatmap, &content, user));
                        if let Some(rank) = play.global_rank {
                            b = b.world_record(rank as u16);
                        }
                        b.build()
                    })
                    .components(vec![score_components(ctx.guild_id())])
            })
            .await?;

            // Save the beatmap...
            cache::save_beatmap(env, ctx.channel_id(), &beatmap).await?;
        }
        Nth::All => {
            let header = format!("Here are the {} plays by {}!", listing_kind, user.mention());
            let reply = ctx.reply(&header).await?;
            style
                .display_scores(
                    plays,
                    ctx.serenity_context(),
                    ctx.guild_id(),
                    (reply, ctx).with_header(header),
                )
                .await?;
        }
    }
    Ok(())
}

/// Get information about a beatmap, or the last beatmap mentioned in the channel.
#[poise::command(slash_command)]
async fn beatmap<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "A link or shortlink to the beatmap or beatmapset"] map: Option<String>,
    #[description = "Override the mods on the map"] mods: Option<UnparsedMods>,
    #[description = "Override the mode of the map"] mode: Option<Mode>,
    #[description = "Load the beatmapset instead"] beatmapset: Option<bool>,
) -> Result<()> {
    let env = ctx.data().osu_env();

    ctx.defer().await?;

    let beatmap = parse_map_input(ctx.channel_id(), env, map, mode, beatmapset).await?;

    // override mods and mode if needed
    match beatmap {
        EmbedType::Beatmap(beatmap, bmode, info, bmmods) => {
            let (beatmap, info, mods) =
                if mods.is_none() && mode.is_none_or(|v| v == bmode.unwrap_or(beatmap.mode)) {
                    (*beatmap, *info, bmmods)
                } else {
                    let mode = bmode.unwrap_or(beatmap.mode);
                    let mods = match mods {
                        None => bmmods,
                        Some(mods) => mods.to_mods(mode)?,
                    };
                    let beatmap = env.beatmaps.get_beatmap(beatmap.beatmap_id, mode).await?;
                    let info = env
                        .oppai
                        .get_beatmap(beatmap.beatmap_id)
                        .await?
                        .get_possible_pp_with(mode, &mods);
                    (beatmap, info, mods)
                };
            ctx.send(
                CreateReply::default()
                    .content(format!("Information for {}", beatmap.mention(mode, &mods)))
                    .embed(beatmap_embed(
                        &beatmap,
                        mode.unwrap_or(beatmap.mode),
                        &mods,
                        &info,
                    ))
                    .components(vec![beatmap_components(
                        mode.unwrap_or(beatmap.mode),
                        ctx.guild_id(),
                    )]),
            )
            .await?;
            let bmode = beatmap.mode.with_override(mode);
            save_beatmap(
                env,
                ctx.channel_id(),
                &BeatmapWithMode(beatmap, Some(bmode)),
            )
            .await?;
        }
        EmbedType::Beatmapset(vec, _) => {
            let b0 = &vec[0];
            let msg = ctx
                .reply(format!("Information for {}", b0.beatmapset_mention()))
                .await?;
            display_beatmapset(
                ctx.serenity_context(),
                vec,
                mode,
                mods,
                ctx.guild_id(),
                (msg, ctx),
            )
            .await?;
        }
    };

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ChoiceParameter, Default)]
enum SortScoreBy {
    #[default]
    PP,
    Score,
    #[name = "Maximum Combo"]
    Combo,
    #[name = "Miss Count"]
    Miss,
    Accuracy,
}

impl SortScoreBy {
    fn compare(self, a: &Score, b: &Score) -> Ordering {
        match self {
            SortScoreBy::PP => {
                b.pp.unwrap_or(0.0)
                    .partial_cmp(&a.pp.unwrap_or(0.0))
                    .unwrap()
            }
            SortScoreBy::Score => b.normalized_score.cmp(&a.normalized_score),
            SortScoreBy::Combo => b.max_combo.cmp(&a.max_combo),
            SortScoreBy::Miss => b.count_miss.cmp(&a.count_miss),
            SortScoreBy::Accuracy => b.server_accuracy.partial_cmp(&a.server_accuracy).unwrap(),
        }
    }
}

/// Display your or a player's scores for a certain beatmap/beatmapset.
#[poise::command(slash_command)]
async fn check<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "A link or shortlink to the beatmap or beatmapset"] map: Option<String>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
    #[description = "Sort scores by"] sort: Option<SortScoreBy>,
    #[description = "Reverse the sorting order"] reverse: Option<bool>,
    #[description = "Filter the mods on the scores"] mods: Option<UnparsedMods>,
    #[description = "Filter the gamemode of the scores"] mode: Option<Mode>,
    #[description = "Find all scores in the beatmapset instead"] beatmapset: Option<bool>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
) -> Result<()> {
    let env = ctx.data().osu_env();

    let user = arg_from_username_or_discord(username, discord_name);
    let args = ListingArgs::from_params(
        env,
        None,
        style.unwrap_or(ScoreListStyle::Grid),
        mode,
        user,
        ctx.author().id,
    )
    .await?;

    ctx.defer().await?;

    let embed = parse_map_input(ctx.channel_id(), env, map, mode, beatmapset).await?;

    let display = embed.mention();

    let ordering = sort.unwrap_or_default();
    let mut scores = do_check(env, &embed, mods, &args.user).await?;
    if scores.is_empty() {
        ctx.reply(format!(
            "No plays found for {} on {} with the required criteria.",
            args.user.mention(),
            display
        ))
        .await?;
        return Ok(());
    }
    scores.sort_unstable_by(|a, b| ordering.compare(a, b));
    if reverse == Some(true) {
        scores.reverse();
    }

    let header = format!(
        "Here are the plays by {} on {}!",
        args.user.mention(),
        display
    );
    let msg = ctx.reply(&header).await?;

    let style = style.unwrap_or(if scores.len() <= 5 {
        ScoreListStyle::Grid
    } else {
        ScoreListStyle::Table
    });

    style
        .display_scores(
            scores,
            ctx.serenity_context(),
            ctx.guild_id(),
            (msg, ctx).with_header(header),
        )
        .await?;

    Ok(())
}

/// Display the rankings of members in the server.
#[poise::command(slash_command, guild_only)]
async fn ranks<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Sort users by"] sort: Option<server_rank::RankQuery>,
    #[description = "Reverse the ordering"] reverse: Option<bool>,
    #[description = "The gamemode for the rankings"] mode: Option<Mode>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let guild = ctx.partial_guild().await.unwrap();
    ctx.defer().await?;
    server_rank::do_server_ranks(
        ctx.serenity_context(),
        env,
        &guild,
        mode,
        sort,
        reverse.unwrap_or(false),
        |s| async move {
            let m = ctx.reply(s).await?;
            Ok(m.into_message().await?)
        },
    )
    .await?;
    Ok(())
}

/// Display the leaderboard on a single map of members in the server.
#[poise::command(slash_command, guild_only)]
async fn leaderboard<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "The link or shortlink of the map"] map: Option<String>,
    #[description = "Load the scoreboard for the entire beatmapset"] beatmapset: Option<bool>,
    #[description = "Sort scores by"] sort: Option<server_rank::OrderBy>,
    #[description = "Reverse the ordering"] reverse: Option<bool>,
    #[description = "Include unranked scores"] unranked: Option<bool>,
    #[description = "Filter the gamemode of the scores"] mode: Option<Mode>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let guild = ctx.partial_guild().await.unwrap();
    let style = style.unwrap_or_default();
    let order = sort.unwrap_or_default();

    let embed = parse_map_input(ctx.channel_id(), env, map, mode, beatmapset).await?;

    ctx.defer().await?;

    let scoreboard_msg = embed.mention();
    let (mut scores, show_diff) = get_leaderboard_from_embed(
        ctx.serenity_context(),
        env,
        embed,
        None,
        unranked.unwrap_or(true),
        order,
        guild.id,
    )
    .await?;
    if reverse == Some(true) {
        scores.reverse();
    }

    if scores.is_empty() {
        ctx.reply(format!(
            "No scores have been recorded in **{}** on {}.",
            guild.name, scoreboard_msg,
        ))
        .await?;
        return Ok(());
    }

    let header = format!(
        "Here are the top scores of **{}** on {}",
        guild.name, scoreboard_msg,
    );
    let has_lazer_score = scores.iter().any(|v| v.score.mods.is_lazer);

    match style {
        ScoreListStyle::Table => {
            let reply = ctx.reply(header).await?.into_message().await?;
            server_rank::display_rankings_table(
                ctx.serenity_context(),
                reply,
                scores,
                has_lazer_score,
                show_diff,
                sort.unwrap_or_default(),
            )
            .await?;
        }
        ScoreListStyle::File => {
            ctx.send(
                CreateReply::default()
                    .content(header)
                    .attachment(CreateAttachment::bytes(
                        server_rank::rankings_to_table(
                            &scores,
                            0,
                            scores.len(),
                            has_lazer_score,
                            show_diff,
                            order,
                        ),
                        "rankings.txt",
                    )),
            )
            .await?;
        }
        ScoreListStyle::Grid => {
            let reply = ctx.reply(header).await?;
            style
                .display_scores(
                    scores.into_iter().map(|s| s.score).collect::<Vec<_>>(),
                    ctx.serenity_context(),
                    Some(guild.id),
                    (reply, ctx),
                )
                .await?;
        }
    }
    Ok(())
}

/// Clear youmu's cache.
#[poise::command(slash_command, owners_only)]
pub async fn clear_cache<U: HasOsuEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Also clear oppai cache"] clear_oppai: bool,
) -> Result<()> {
    let env = ctx.data().osu_env();
    ctx.defer_ephemeral().await?;

    env.beatmaps.clear().await?;

    if clear_oppai {
        env.oppai.clear().await?;
    }
    ctx.reply("Beatmap cache cleared!").await?;
    Ok(())
}

fn arg_from_username_or_discord(
    username: Option<String>,
    discord_name: Option<User>,
) -> Option<UsernameArg> {
    match (username, discord_name) {
        (Some(v), _) => Some(UsernameArg::Raw(v)),
        (_, Some(u)) => Some(UsernameArg::Tagged(u.id)),
        (None, None) => None,
    }
}

async fn parse_map_input(
    channel_id: serenity::all::ChannelId,
    env: &OsuEnv,
    input: Option<String>,
    mode: Option<Mode>,
    beatmapset: Option<bool>,
) -> Result<EmbedType> {
    let output = match input {
        None => {
            let Some(v) = load_beatmap_from_channel(env, channel_id).await else {
                return Err(Error::msg("no beatmap mentioned in this channel"));
            };
            v
        }
        Some(map) => {
            if let Ok(id) = map.parse::<u64>() {
                let beatmap = match mode {
                    None => env.beatmaps.get_beatmap_default(id).await,
                    Some(mode) => env.beatmaps.get_beatmap(id, mode).await,
                }?;
                let info = env
                    .oppai
                    .get_beatmap(beatmap.beatmap_id)
                    .await?
                    .get_possible_pp_with(beatmap.mode, Mods::NOMOD);
                return Ok(EmbedType::Beatmap(
                    Box::new(beatmap),
                    None,
                    Box::new(info),
                    Mods::NOMOD.clone(),
                ));
            }
            let Some(results) = stream::select(
                link_parser::parse_new_links(env, &map),
                stream::select(
                    link_parser::parse_old_links(env, &map),
                    link_parser::parse_short_links(env, &map),
                ),
            )
            .next()
            .await
            else {
                return Err(Error::msg("no beatmap detected in the argument"));
            };
            results.embed
        }
    };

    // override into beatmapset if needed
    let output = if beatmapset == Some(true) {
        match output {
            EmbedType::Beatmap(beatmap, _, _, _) => {
                let beatmaps = env
                    .beatmaps
                    .get_beatmapset(beatmap.beatmapset_id, mode)
                    .await?;
                EmbedType::Beatmapset(beatmaps, mode)
            }
            bm @ EmbedType::Beatmapset(_, _) => bm,
        }
    } else {
        output
    };

    Ok(output)
}

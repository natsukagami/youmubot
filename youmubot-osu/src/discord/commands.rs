use super::*;
use poise::CreateReply;
use serenity::all::User;

/// osu!-related command group.
#[poise::command(slash_command, subcommands("profile", "top", "recent", "save"))]
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
    #[max = 100]
    index: Option<u8>,
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
    #[description = "Game mode"] mode: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let username_arg = arg_from_username_or_discord(username, discord_name);
    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = ListingArgs::from_params(
        env,
        index,
        style.unwrap_or(ScoreListStyle::Table),
        mode,
        username_arg,
        ctx.author().id,
    )
    .await?;
    let osu_client = &env.client;

    ctx.defer().await?;

    let mut plays = osu_client
        .user_best(UserID::ID(user.id), |f| f.mode(mode).limit(100))
        .await?;

    plays.sort_unstable_by(|a, b| b.pp.partial_cmp(&a.pp).unwrap());
    let plays = plays;

    match nth {
        Nth::Nth(nth) => {
            let Some(play) = plays.get(nth as usize) else {
                Err(Error::msg("no such play"))?
            };

            let beatmap = env.beatmaps.get_beatmap(play.beatmap_id, mode).await?;
            let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
            let beatmap = BeatmapWithMode(beatmap, mode);

            ctx.send({
                CreateReply::default()
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
                    .components(vec![score_components(ctx.guild_id())])
            })
            .await?;

            // Save the beatmap...
            cache::save_beatmap(&env, ctx.channel_id(), &beatmap).await?;
        }
        Nth::All => {
            let reply = ctx
                .clone()
                .reply(format!("Here are the top plays by {}!", user.mention()))
                .await?
                .into_message()
                .await?;
            style
                .display_scores(plays, mode, ctx.serenity_context(), ctx.guild_id(), reply)
                .await?;
        }
    }
    Ok(())
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
    #[description = "Score listing style"] style: Option<ScoreListStyle>,
    #[description = "Game mode"] mode: Option<Mode>,
    #[description = "osu! username"] username: Option<String>,
    #[description = "Discord username"] discord_name: Option<User>,
) -> Result<()> {
    let env = ctx.data().osu_env();
    let args = arg_from_username_or_discord(username, discord_name);
    let style = style.unwrap_or(ScoreListStyle::Table);

    let ListingArgs {
        nth,
        style,
        mode,
        user,
    } = ListingArgs::from_params(env, index, style, mode, args, ctx.author().id).await?;

    ctx.defer().await?;

    let osu_client = &env.client;
    let plays = osu_client
        .user_recent(UserID::ID(user.id), |f| f.mode(mode).limit(50))
        .await?;
    match nth {
        Nth::All => {
            let reply = ctx
                .clone()
                .reply(format!("Here are the recent plays by {}!", user.mention()))
                .await?
                .into_message()
                .await?;
            style
                .display_scores(plays, mode, ctx.serenity_context(), reply.guild_id, reply)
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

            ctx.send(
                CreateReply::default()
                    .content(format!(
                        "Here is the #{} recent play by {}!",
                        nth + 1,
                        user.mention()
                    ))
                    .embed(
                        score_embed(play, &beatmap_mode, &content, user)
                            .footer(format!("Attempt #{}", attempts))
                            .build(),
                    )
                    .components(vec![score_components(ctx.guild_id())]),
            )
            .await?;

            // Save the beatmap...
            cache::save_beatmap(&env, ctx.channel_id(), &beatmap_mode).await?;
        }
    }
    Ok(())
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
        .clone()
        .send(
            CreateReply::default()
                .content(format!(
                    "To set your osu username to **{}**, please make your most recent play \
            be the following map: `/b/{}` in **{}** mode! \
        It does **not** have to be a pass, and **NF** can be used! \
        React to this message with 👌 within 5 minutes when you're done!",
                    u.username,
                    score.beatmap_id,
                    mode.as_str_new_site()
                ))
                .embed(beatmap_embed(&beatmap, mode, Mods::NOMOD, &info))
                .components(vec![beatmap_components(mode, ctx.guild_id())]),
        )
        .await?
        .into_message()
        .await?;
    handle_save_respond(
        ctx.serenity_context(),
        &env,
        ctx.author().id,
        reply,
        &beatmap,
        u,
        mode,
    )
    .await?;
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

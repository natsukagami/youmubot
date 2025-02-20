use std::{pin::Pin, str::FromStr, time::Duration};

use future::Future;
use serenity::all::{
    ComponentInteraction, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateInputText, CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage, CreateQuickModal, GuildId, InputTextStyle, Interaction,
    QuickModalResponse,
};
use youmubot_prelude::*;

use crate::{discord::embeds::FakeScore, mods::UnparsedMods, Mode, Mods, UserHeader};

use super::{
    display::ScoreListStyle,
    embeds::beatmap_embed,
    link_parser::EmbedType,
    server_rank::{display_rankings_table, get_leaderboard_from_embed, OrderBy},
    BeatmapWithMode, LoadRequest, OsuEnv,
};

pub(super) const BTN_CHECK: &str = "youmubot_osu_btn_check";
pub(super) const BTN_LB: &str = "youmubot_osu_btn_lb";
pub(super) const BTN_LAST: &str = "youmubot_osu_btn_last";
pub(super) const BTN_LAST_SET: &str = "youmubot_osu_btn_last_set";
pub(super) const BTN_SIMULATE: &str = "youmubot_osu_btn_simulate";

/// Create an action row for score pages.
pub fn score_components(guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button(), last_button()];
    if guild_id.is_some() {
        btns.insert(1, lb_button());
    }
    CreateActionRow::Buttons(btns)
}

/// Create an action row for score pages.
pub fn beatmap_components(mode: Mode, guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button()];
    if guild_id.is_some() {
        btns.push(lb_button());
    }
    btns.push(mapset_button());
    if mode == Mode::Std {
        btns.push(simulate_button());
    }
    CreateActionRow::Buttons(btns)
}

/// Creates a new check button.
pub fn check_button() -> CreateButton {
    CreateButton::new(BTN_CHECK)
        .label("Me")
        .emoji('🔎')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `check` button on scores and beatmaps.
pub fn handle_check_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_CHECK).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
        let embed = super::load_beatmap(&env, comp.channel_id, Some(msg), LoadRequest::Any)
            .await
            .unwrap();
        let user = match env.saved_users.by_user_id(comp.user.id).await? {
            Some(u) => u,
            None => {
                comp.create_followup(&ctx, CreateInteractionResponseFollowup::new().content("You don't have a saved account yet! Save your osu! account by `y2!osu save <your-osu-username>`.").ephemeral(true)).await?;
                return Ok(());
            }
        };
        let header = UserHeader::from(user.clone());

        let scores = super::do_check(&env, &embed, None, &header).await?;
        if scores.is_empty() {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "No plays found for [`{}`](<https://osu.ppy.sh/users/{}>) on {}.",
                    user.username,
                    user.id,
                    embed.mention(),
                )),
            )
            .await?;
            return Ok(());
        }

        comp.create_followup(
            &ctx,
            CreateInteractionResponseFollowup::new().content(format!(
                "Here are the scores by [`{}`](<https://osu.ppy.sh/users/{}>) on {}!",
                user.username,
                user.id,
                embed.mention()
            )),
        )
        .await?;

        let guild_id = comp.guild_id;
        ScoreListStyle::Grid
            .display_scores(scores, &ctx, guild_id, (comp, ctx))
            .await
            .pls_ok();
        Ok(())
    })
}

/// Creates a new check button.
pub fn last_button() -> CreateButton {
    CreateButton::new(BTN_LAST)
        .label("Map")
        .emoji('🎼')
        .style(serenity::all::ButtonStyle::Success)
}

pub fn mapset_button() -> CreateButton {
    CreateButton::new(BTN_LAST_SET)
        .label("Set")
        .emoji('📚')
        .style(serenity::all::ButtonStyle::Success)
}

pub fn simulate_button() -> CreateButton {
    CreateButton::new(BTN_SIMULATE)
        .label("What If?")
        .emoji('🌈')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `last` button on scores and beatmaps.
pub fn handle_last_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LAST).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        handle_last_req(ctx, comp, false).await
    })
}

/// Implements the `beatmapset` button on scores and beatmaps.
pub fn handle_last_set_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LAST_SET).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        handle_last_req(ctx, comp, true).await
    })
}

/// Implements the `simulate` button on beatmaps.
pub fn handle_simulate_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_SIMULATE
                    && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
            {
                comp
            }
            _ => return Ok(()),
        };

        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

        let embed = super::load_beatmap(&env, comp.channel_id, Some(msg), LoadRequest::Beatmap)
            .await
            .unwrap();
        let (b, mode) = match embed {
            EmbedType::Beatmap(beatmap, mode, _, _) => {
                let mode = mode.unwrap_or(beatmap.mode);
                (beatmap, mode)
            }
            EmbedType::Beatmapset(_, _) => return Err(Error::msg("Cannot find any beatmap")),
        };
        let content = env.oppai.get_beatmap(b.beatmap_id).await?;
        let info = content.get_info_with(mode, Mods::NOMOD);

        assert!(mode == Mode::Std);

        fn mk_input(title: &str, placeholder: impl Into<String>) -> CreateInputText {
            CreateInputText::new(InputTextStyle::Short, title, "")
                .placeholder(placeholder)
                .required(false)
        }

        let Some(query) = comp
            .quick_modal(
                &ctx,
                CreateQuickModal::new(format!(
                    "Simulate Score on beatmap `{}`",
                    b.short_link(None, Mods::NOMOD)
                ))
                .timeout(Duration::from_secs(300))
                .field(mk_input("Mods", "NM"))
                .field(mk_input("Max Combo", info.attrs.max_combo().to_string()))
                .field(mk_input("100s", "0"))
                .field(mk_input("50s", "0"))
                .field(mk_input("Misses", "0")),
                // .short_field("Slider Ends Missed (Lazer Only)"), // too long LMAO
            )
            .await?
        else {
            return Ok(());
        };

        query.interaction.defer(&ctx).await?;

        if let Err(err) =
            handle_simluate_query(ctx, &env, &query, BeatmapWithMode(*b, Some(mode))).await
        {
            query
                .interaction
                .create_followup(
                    ctx,
                    CreateInteractionResponseFollowup::new()
                        .content(format!("Cannot simulate score: {}", err))
                        .ephemeral(true),
                )
                .await
                .pls_ok();
        }

        Ok(())
    })
}

async fn handle_simluate_query(
    ctx: &Context,
    env: &OsuEnv,
    query: &QuickModalResponse,
    bm: BeatmapWithMode,
) -> Result<()> {
    let b = &bm.0;
    let mode = bm.1.unwrap_or(b.mode);
    let content = env.oppai.get_beatmap(b.beatmap_id).await?;

    let score: FakeScore = {
        let inputs = &query.inputs;
        let (mods, max_combo, c100, c50, cmiss) =
            (&inputs[0], &inputs[1], &inputs[2], &inputs[3], &inputs[4]);
        let mods = UnparsedMods::from_str(mods)
            .map_err(|v| Error::msg(v))?
            .to_mods(mode)?;
        let info = content.get_info_with(mode, &mods);
        let max_combo = max_combo.parse::<u32>().ok();
        let n100 = c100.parse::<u32>().unwrap_or(0);
        let n50 = c50.parse::<u32>().unwrap_or(0);
        let nmiss = cmiss.parse::<u32>().unwrap_or(0);
        let n300 = info.object_count as u32 - n100 - n50 - nmiss;
        FakeScore {
            bm: &bm,
            content: &content,
            mods,
            n300,
            n100,
            n50,
            nmiss,
            max_combo,
        }
    };

    query
        .interaction
        .create_followup(
            &ctx,
            CreateInteractionResponseFollowup::new()
                .content(format!(
                    "Simulated score for `{}`",
                    b.short_link(None, Mods::NOMOD)
                ))
                .add_embed(score.embed(ctx)?)
                .components(vec![score_components(query.interaction.guild_id)]),
        )
        .await?;

    Ok(())
}

async fn handle_last_req(
    ctx: &Context,
    comp: &ComponentInteraction,
    is_beatmapset_req: bool,
) -> Result<()> {
    let msg = &*comp.message;

    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let embed = super::load_beatmap(
        &env,
        comp.channel_id,
        Some(msg),
        if is_beatmapset_req {
            LoadRequest::Beatmapset
        } else {
            LoadRequest::Any
        },
    )
    .await
    .unwrap();

    let content_type = format!("Information for {}", embed.mention());
    match embed {
        EmbedType::Beatmapset(beatmapset, mode) => {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(content_type),
            )
            .await?;
            super::display::display_beatmapset(
                ctx,
                beatmapset,
                mode,
                None,
                comp.guild_id,
                (comp, ctx),
            )
            .await?;
            return Ok(());
        }
        EmbedType::Beatmap(b, m, _, mods) => {
            let info = env
                .oppai
                .get_beatmap(b.beatmap_id)
                .await?
                .get_possible_pp_with(m.unwrap_or(b.mode), &mods);
            comp.create_followup(
                &ctx,
                serenity::all::CreateInteractionResponseFollowup::new()
                    .content(content_type)
                    .embed(beatmap_embed(&*b, m.unwrap_or(b.mode), &mods, &info))
                    .components(vec![beatmap_components(m.unwrap_or(b.mode), comp.guild_id)]),
            )
            .await?;
            // Save the beatmap...
            super::cache::save_beatmap(&env, msg.channel_id, &BeatmapWithMode(*b, m)).await?;
        }
    }

    Ok(())
}

/// Creates a new check button.
pub fn lb_button() -> CreateButton {
    CreateButton::new(BTN_LB)
        .label("Ranks")
        .emoji('👑')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `lb` button on scores and beatmaps.
pub fn handle_lb_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LB).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

        let embed = super::load_beatmap(&env, comp.channel_id, Some(msg), LoadRequest::Any)
            .await
            .unwrap();
        let order = OrderBy::default();
        let guild = comp.guild_id.expect("Guild-only command");

        let scoreboard_msg = embed.mention();
        let (scores, show_diff) =
            get_leaderboard_from_embed(ctx, &env, embed, None, false, order, guild).await?;

        if scores.is_empty() {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "No scores have been recorded for {} from anyone in this server.",
                    scoreboard_msg
                )),
            )
            .await?;
            return Ok(());
        }

        let reply = comp
            .create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new()
                    .content(format!("Here are the top scores on {}!", scoreboard_msg)),
            )
            .await?;
        display_rankings_table(ctx, reply, scores, show_diff, order).await?;
        Ok(())
    })
}

async fn expect_and_defer_button<'a>(
    ctx: &'_ Context,
    interaction: &'a Interaction,
    expect_id: &'static str,
) -> Result<Option<&'a ComponentInteraction>> {
    let comp = match interaction.as_message_component() {
        Some(comp)
            if comp.data.custom_id == expect_id
                && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
        {
            comp
        }
        _ => return Ok(None),
    };
    comp.create_response(
        &ctx,
        CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
    )
    .await?;
    Ok(Some(comp))
}

use std::pin::Pin;

use future::Future;
use serenity::all::{
    ComponentInteractionDataKind, CreateActionRow, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, GuildId, Interaction,
};
use youmubot_prelude::*;

use crate::Mods;

use super::{
    display::ScoreListStyle,
    embeds::beatmap_embed,
    server_rank::{display_rankings_table, get_leaderboard, OrderBy},
    BeatmapWithMode, OsuEnv,
};

pub(super) const BTN_CHECK: &'static str = "youmubot_osu_btn_check";
pub(super) const BTN_LB: &'static str = "youmubot_osu_btn_lb";
pub(super) const BTN_LAST: &'static str = "youmubot_osu_btn_last";

/// Create an action row for score pages.
pub fn score_components(guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button(), last_button()];
    if guild_id.is_some() {
        btns.insert(1, lb_button());
    }
    CreateActionRow::Buttons(btns)
}

/// Create an action row for score pages.
pub fn beatmap_components(guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button()];
    if guild_id.is_some() {
        btns.push(lb_button());
    }
    CreateActionRow::Buttons(btns)
}

/// Creates a new check button.
pub fn check_button() -> CreateButton {
    CreateButton::new(BTN_CHECK)
        .label("Check")
        .emoji('🔎')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `check` button on scores and beatmaps.
pub fn handle_check_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_CHECK
                    && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
            {
                comp
            }
            _ => return Ok(()),
        };
        let (msg, author) = (&*comp.message, comp.user.id);

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
        let (bm, _) = super::load_beatmap(&env, comp.channel_id, Some(msg))
            .await
            .unwrap();
        let user_id = super::to_user_id_query(None, &env, author).await?;

        let scores = super::do_check(&env, &bm, Mods::NOMOD, &user_id).await?;

        let reply = {
            comp.create_response(
                &ctx,
                serenity::all::CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content(format!(
                        "Here are the scores by `{}` on `{}`!",
                        &user_id,
                        bm.short_link(Mods::NOMOD)
                    )),
                ),
            )
            .await?;
            comp.get_response(&ctx).await?
        };
        ScoreListStyle::Grid
            .display_scores(scores, bm.1, ctx, comp.guild_id, reply)
            .await?;

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

/// Implements the `last` button on scores and beatmaps.
pub fn handle_last_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_LAST
                    && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
            {
                comp
            }
            _ => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

        let (BeatmapWithMode(b, m), mods_def) =
            super::load_beatmap(&env, comp.channel_id, Some(msg))
                .await
                .unwrap();

        let mods = mods_def.unwrap_or(Mods::NOMOD);
        let info = env
            .oppai
            .get_beatmap(b.beatmap_id)
            .await?
            .get_possible_pp_with(m, mods)?;
        comp.create_response(
            &ctx,
            serenity::all::CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("Here is the beatmap you requested!")
                    .embed(beatmap_embed(&b, m, mods, info))
                    .components(vec![beatmap_components(comp.guild_id)]),
            ),
        )
        .await?;
        // Save the beatmap...
        super::cache::save_beatmap(&env, msg.channel_id, &BeatmapWithMode(b, m)).await?;

        Ok(())
    })
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
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_LB
                    && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
            {
                comp
            }
            _ => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

        let (bm, _) = super::load_beatmap(&env, comp.channel_id, Some(msg))
            .await
            .unwrap();
        let order = OrderBy::default();
        let guild = comp.guild_id.expect("Guild-only command");

        comp.create_response(
            &ctx,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;
        let scores = get_leaderboard(&ctx, &env, &bm, order, guild).await?;

        if scores.is_empty() {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new()
                    .content("No scores have been recorded for this beatmap."),
            )
            .await?;
            return Ok(());
        }

        let reply = comp
            .create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "⌛ Loading top scores on beatmap `{}`...",
                    bm.short_link(Mods::NOMOD)
                )),
            )
            .await?;
        display_rankings_table(&ctx, reply, scores, &bm, order).await?;
        Ok(())
    })
}

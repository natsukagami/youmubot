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
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_CHECK
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
        let user = match env.saved_users.by_user_id(comp.user.id).await? {
            Some(u) => u,
            None => {
                comp.create_response(&ctx, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content("You don't have a saved account yet! Save your osu! account by `y2!osu save <your-osu-username>`.").ephemeral(true))).await?;
                return Ok(());
            }
        };

        let scores = super::do_check(&env, &bm, Mods::NOMOD, &crate::UserID::ID(user.id)).await?;
        if scores.is_empty() {
            comp.create_response(
                &ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content(format!(
                        "No plays found for [`{}`](<https://osu.ppy.sh/users/{}>) on `{}`.",
                        user.username,
                        user.id,
                        bm.short_link(Mods::NOMOD)
                    )),
                ),
            )
            .await?;
            return Ok(());
        }

        let reply = {
            comp.create_response(
                &ctx,
                serenity::all::CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content(format!(
                        "Here are the scores by [`{}`](<https://osu.ppy.sh/users/{}>) on `{}`!",
                        user.username,
                        user.id,
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

        let (bm, mods_def) = super::load_beatmap(&env, comp.channel_id, Some(msg))
            .await
            .unwrap();
        let BeatmapWithMode(b, m) = &bm;

        let mods = mods_def.unwrap_or(Mods::NOMOD);
        let info = env
            .oppai
            .get_beatmap(b.beatmap_id)
            .await?
            .get_possible_pp_with(*m, mods)?;
        comp.create_response(
            &ctx,
            serenity::all::CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("Information for beatmap `{}`", bm.short_link(mods)))
                    .embed(beatmap_embed(b, *m, mods, info))
                    .components(vec![beatmap_components(comp.guild_id)]),
            ),
        )
        .await?;
        // Save the beatmap...
        super::cache::save_beatmap(&env, msg.channel_id, &bm).await?;

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
                CreateInteractionResponseFollowup::new().content(
                    "No scores have been recorded for this beatmap from anyone in this server.",
                ),
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

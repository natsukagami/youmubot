use super::BeatmapWithMode;
use serenity::model::id::ChannelId;
use youmubot_prelude::*;

/// Save the beatmap into the server data storage.
pub(crate) async fn save_beatmap(
    env: &crate::discord::Env,
    channel_id: ChannelId,
    bm: &BeatmapWithMode,
) -> Result<()> {
    env.last_beatmaps.save(channel_id, &bm.0, bm.1).await?;

    Ok(())
}

/// Get the last beatmap requested from this channel.
pub(crate) async fn get_beatmap(
    env: &crate::discord::Env,
    channel_id: ChannelId,
) -> Result<Option<BeatmapWithMode>> {
    env.last_beatmaps
        .by_channel(channel_id)
        .await
        .map(|v| v.map(|(bm, mode)| BeatmapWithMode(bm, mode)))
}

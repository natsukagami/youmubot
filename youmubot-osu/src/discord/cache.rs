use super::db::OsuLastBeatmap;
use super::BeatmapWithMode;
use serenity::model::id::ChannelId;
use youmubot_prelude::*;

/// Save the beatmap into the server data storage.
pub(crate) async fn save_beatmap(
    data: &TypeMap,
    channel_id: ChannelId,
    bm: &BeatmapWithMode,
) -> Result<()> {
    data.get::<OsuLastBeatmap>()
        .unwrap()
        .save(channel_id, &bm.0, bm.1)
        .await?;

    Ok(())
}

/// Get the last beatmap requested from this channel.
pub(crate) async fn get_beatmap(
    data: &TypeMap,
    channel_id: ChannelId,
) -> Result<Option<BeatmapWithMode>> {
    data.get::<OsuLastBeatmap>()
        .unwrap()
        .by_channel(channel_id)
        .await
        .map(|v| v.map(|(bm, mode)| BeatmapWithMode(bm, mode)))
}

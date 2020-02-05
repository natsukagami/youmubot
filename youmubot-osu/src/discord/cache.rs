use super::db::OsuLastBeatmap;
use super::BeatmapWithMode;
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    model::id::ChannelId,
    prelude::*,
};

/// Save the beatmap into the server data storage.
pub(crate) fn save_beatmap(
    data: &ShareMap,
    channel_id: ChannelId,
    bm: &BeatmapWithMode,
) -> CommandResult {
    let db = OsuLastBeatmap::open(data);
    let mut db = db.borrow_mut()?;

    db.insert(channel_id, (bm.0.clone(), bm.mode()));

    Ok(())
}

/// Get the last beatmap requested from this channel.
pub(crate) fn get_beatmap(
    data: &ShareMap,
    channel_id: ChannelId,
) -> Result<Option<BeatmapWithMode>, Error> {
    let db = OsuLastBeatmap::open(data);
    let db = db.borrow()?;

    Ok(db
        .get(&channel_id)
        .cloned()
        .map(|(a, b)| BeatmapWithMode(a, b)))
}

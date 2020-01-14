use super::BeatmapWithMode;
use crate::db::{DBWriteGuard, OsuLastBeatmap};
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    model::id::ChannelId,
    prelude::*,
};
use youmubot_osu::models::Mode;

/// Save the beatmap into the server data storage.
pub(crate) fn save_beatmap(
    data: &mut ShareMap,
    channel_id: ChannelId,
    bm: &BeatmapWithMode,
) -> CommandResult {
    let mut db: DBWriteGuard<_> = data
        .get_mut::<OsuLastBeatmap>()
        .expect("DB is implemented")
        .into();
    let mut db = db.borrow_mut()?;

    db.insert(channel_id, (bm.0.clone(), bm.mode()));

    Ok(())
}

/// Get the last beatmap requested from this channel.
pub(crate) fn get_beatmap(
    data: &ShareMap,
    channel_id: ChannelId,
) -> Result<Option<BeatmapWithMode>, Error> {
    let db = data.get::<OsuLastBeatmap>().expect("DB is implemented");
    let db = db.borrow_data()?;

    Ok(db
        .get(&channel_id)
        .cloned()
        .map(|(a, b)| BeatmapWithMode(a, b)))
}

use rosu_v2::model as rosu;

use super::*;

impl ApprovalStatus {
    pub(crate) fn from_rosu(
        rank_status: rosu_v2::model::beatmap::RankStatus,
        ranked_date: Option<DateTime<Utc>>,
    ) -> Self {
        use ApprovalStatus::*;
        match rank_status {
            rosu_v2::model::beatmap::RankStatus::Graveyard => Graveyarded,
            rosu_v2::model::beatmap::RankStatus::WIP => WIP,
            rosu_v2::model::beatmap::RankStatus::Pending => Pending,
            rosu_v2::model::beatmap::RankStatus::Ranked => Ranked(ranked_date.unwrap()),
            rosu_v2::model::beatmap::RankStatus::Approved => Approved,
            rosu_v2::model::beatmap::RankStatus::Qualified => Qualified,
            rosu_v2::model::beatmap::RankStatus::Loved => Loved,
        }
    }
}

fn time_to_utc(s: time::OffsetDateTime) -> DateTime<Utc> {
    chrono::DateTime::from_timestamp(s.unix_timestamp(), 0).unwrap()
}

impl Beatmap {
    pub(crate) fn from_rosu(bm: rosu::beatmap::Beatmap, set: &rosu::beatmap::Beatmapset) -> Self {
        let last_updated = time_to_utc(bm.last_updated);
        let difficulty = Difficulty::from_rosu(&bm);
        Self {
            approval: ApprovalStatus::from_rosu(bm.status, set.ranked_date.map(time_to_utc)),
            submit_date: set.submitted_date.map(time_to_utc).unwrap_or(last_updated),
            last_update: last_updated,
            download_available: !set.availability.download_disabled, // don't think we have this stat
            audio_available: !set.availability.download_disabled,    // neither is this
            artist: set.artist_unicode.as_ref().unwrap_or(&set.artist).clone(),
            title: set.title_unicode.as_ref().unwrap_or(&set.title).clone(),
            beatmapset_id: set.mapset_id as u64,
            creator: set.creator_name.clone().into_string(),
            creator_id: set.creator_id as u64,
            source: Some(set.source.clone()).filter(|s| s != "").clone(),
            genre: set.genre.map(|v| v.into()).unwrap_or(Genre::Unspecified),
            language: set.language.map(|v| v.into()).unwrap_or(Language::Any),
            tags: set.tags.split(", ").map(|v| v.to_owned()).collect(),
            beatmap_id: bm.map_id as u64,
            difficulty_name: bm.version,
            difficulty,
            file_hash: bm.checksum.unwrap_or_else(|| "none".to_owned()),
            mode: bm.mode.into(),
            favourite_count: set.favourite_count as u64,
            rating: set
                .ratings
                .as_ref()
                .map(|rs| {
                    (rs.iter()
                        .enumerate()
                        .map(|(r, id)| ((r + 1) as u32 * *id))
                        .sum::<u32>()) as f64
                        / (rs.iter().sum::<u32>() as f64)
                })
                .unwrap_or(0.0),
            play_count: bm.playcount as u64,
            pass_count: bm.passcount as u64,
        }
    }
}

impl Difficulty {
    pub(crate) fn from_rosu(bm: &rosu::beatmap::Beatmap) -> Self {
        Self {
            stars: bm.stars as f64,
            aim: None,
            speed: None,
            cs: bm.cs as f64,
            od: bm.od as f64,
            ar: bm.ar as f64,
            hp: bm.hp as f64,
            count_normal: bm.count_circles as u64,
            count_slider: bm.count_sliders as u64,
            count_spinner: bm.count_sliders as u64,
            max_combo: bm.max_combo.map(|v| v as u64),
            bpm: bm.bpm as f64,
            drain_length: Duration::from_secs(bm.seconds_drain as u64),
            total_length: Duration::from_secs(bm.seconds_total as u64),
        }
    }
}

impl From<rosu::GameMode> for Mode {
    fn from(value: rosu::GameMode) -> Self {
        match value {
            rosu::GameMode::Osu => Mode::Std,
            rosu::GameMode::Taiko => Mode::Taiko,
            rosu::GameMode::Catch => Mode::Catch,
            rosu::GameMode::Mania => Mode::Mania,
        }
    }
}

impl From<rosu::beatmap::Genre> for Genre {
    fn from(value: rosu::beatmap::Genre) -> Self {
        match value {
            rosu::beatmap::Genre::Any => Genre::Any,
            rosu::beatmap::Genre::Unspecified => Genre::Unspecified,
            rosu::beatmap::Genre::VideoGame => Genre::VideoGame,
            rosu::beatmap::Genre::Anime => Genre::Anime,
            rosu::beatmap::Genre::Rock => Genre::Rock,
            rosu::beatmap::Genre::Pop => Genre::Pop,
            rosu::beatmap::Genre::Other => Genre::Other,
            rosu::beatmap::Genre::Novelty => Genre::Novelty,
            rosu::beatmap::Genre::HipHop => Genre::HipHop,
            rosu::beatmap::Genre::Electronic => Genre::Electronic,
            rosu::beatmap::Genre::Metal => Genre::Metal,
            rosu::beatmap::Genre::Classical => Genre::Classical,
            rosu::beatmap::Genre::Folk => Genre::Folk,
            rosu::beatmap::Genre::Jazz => Genre::Jazz,
        }
    }
}

impl From<rosu::beatmap::Language> for Language {
    fn from(value: rosu::beatmap::Language) -> Self {
        match value {
            rosu::beatmap::Language::Any => Language::Any,
            rosu::beatmap::Language::Other => Language::Other,
            rosu::beatmap::Language::English => Language::English,
            rosu::beatmap::Language::Japanese => Language::Japanese,
            rosu::beatmap::Language::Chinese => Language::Chinese,
            rosu::beatmap::Language::Instrumental => Language::Instrumental,
            rosu::beatmap::Language::Korean => Language::Korean,
            rosu::beatmap::Language::French => Language::French,
            rosu::beatmap::Language::German => Language::German,
            rosu::beatmap::Language::Swedish => Language::Swedish,
            rosu::beatmap::Language::Spanish => Language::Spanish,
            rosu::beatmap::Language::Italian => Language::Italian,
            rosu::beatmap::Language::Russian => Language::Russian,
            rosu::beatmap::Language::Polish => Language::Polish,
            rosu::beatmap::Language::Unspecified => Language::Unspecified,
        }
    }
}

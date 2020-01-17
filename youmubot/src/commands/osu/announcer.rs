use super::{embeds::score_embed, BeatmapWithMode};
use crate::{
    commands::announcer::Announcer,
    db::{OsuSavedUsers, OsuUser},
    http::{Osu, HTTP},
};
use reqwest::blocking::Client as HTTPClient;
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    http::Http,
    model::{
        id::{ChannelId, UserId},
        misc::Mentionable,
    },
    prelude::ShareMap,
};
use youmubot_osu::{
    models::{Mode, Score},
    request::{BeatmapRequestKind, UserID},
    Client as OsuClient,
};

/// Announce osu! top scores.
pub struct OsuAnnouncer;

impl Announcer for OsuAnnouncer {
    fn announcer_key() -> &'static str {
        "osu"
    }
    fn send_messages(
        c: &Http,
        d: &mut ShareMap,
        channels: impl Fn(UserId) -> Vec<ChannelId>,
    ) -> CommandResult {
        let http = d.get::<HTTP>().expect("HTTP");
        let osu = d.get::<Osu>().expect("osu!client");
        // For each user...
        let mut data = d
            .get::<OsuSavedUsers>()
            .expect("DB initialized")
            .read(|f| f.clone())?;
        for (user_id, osu_user) in data.iter_mut() {
            let mut user = None;
            for mode in &[Mode::Std, Mode::Taiko, Mode::Mania, Mode::Catch] {
                let scores = OsuAnnouncer::scan_user(http, osu, osu_user, *mode)?;
                if scores.is_empty() {
                    continue;
                }
                let user = user.get_or_insert_with(|| {
                    osu.user(http, UserID::ID(osu_user.id), |f| f)
                        .unwrap()
                        .unwrap()
                });
                for (rank, score) in scores {
                    let beatmap = BeatmapWithMode(
                        osu.beatmaps(http, BeatmapRequestKind::Beatmap(score.beatmap_id), |f| f)?
                            .into_iter()
                            .next()
                            .unwrap(),
                        *mode,
                    );
                    for channel in channels(*user_id) {
                        channel.send_message(c, |c| {
                            c.content(format!("New top record from {}!", user_id.mention()))
                                .embed(|e| score_embed(&score, &beatmap, &user, Some(rank), e))
                        })?;
                    }
                }
            }
            osu_user.last_update = chrono::Utc::now();
        }
        // Update users
        let f = d.get_mut::<OsuSavedUsers>().expect("DB initialized");
        f.write(|f| *f = data)?;
        f.save()?;
        Ok(())
    }
}

impl OsuAnnouncer {
    fn scan_user(
        http: &HTTPClient,
        osu: &OsuClient,
        u: &OsuUser,
        mode: Mode,
    ) -> Result<Vec<(u8, Score)>, Error> {
        let scores = osu.user_best(http, UserID::ID(u.id), |f| f.mode(mode).limit(25))?;
        let scores = scores
            .into_iter()
            .filter(|s: &Score| s.date >= u.last_update)
            .enumerate()
            .map(|(i, v)| ((i + 1) as u8, v))
            .collect();
        Ok(scores)
    }
}

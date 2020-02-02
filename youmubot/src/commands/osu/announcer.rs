use super::{embeds::score_embed, BeatmapWithMode};
use crate::{
    commands::announcer::Announcer,
    db::{OsuSavedUsers, OsuUser},
    http::{Osu, HTTP},
};
use rayon::prelude::*;
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
        channels: impl Fn(UserId) -> Vec<ChannelId> + Sync,
    ) -> CommandResult {
        let osu = d.get::<Osu>().expect("osu!client").clone();
        // For each user...
        let mut data = d
            .get::<OsuSavedUsers>()
            .expect("DB initialized")
            .read(|f| f.clone())?;
        for (user_id, osu_user) in data.iter_mut() {
            let mut user = None;
            for mode in &[Mode::Std, Mode::Taiko, Mode::Mania, Mode::Catch] {
                let scores = OsuAnnouncer::scan_user(&osu, osu_user, *mode)?;
                if scores.is_empty() {
                    continue;
                }
                let user = {
                    let user = &mut user;
                    if let None = user {
                        match osu.user(UserID::ID(osu_user.id), |f| f.mode(*mode)) {
                            Ok(u) => {
                                *user = u;
                            }
                            Err(_) => continue,
                        }
                    };
                    user.as_ref().unwrap()
                };
                scores
                    .into_par_iter()
                    .filter_map(|(rank, score)| {
                        let beatmap = osu
                            .beatmaps(BeatmapRequestKind::Beatmap(score.beatmap_id), |f| f)
                            .map(|v| BeatmapWithMode(v.into_iter().next().unwrap(), *mode));
                        let channels = channels(*user_id);
                        match beatmap {
                            Ok(v) => Some((rank, score, v, channels)),
                            Err(e) => {
                                dbg!(e);
                                None
                            }
                        }
                    })
                    .for_each(|(rank, score, beatmap, channels)| {
                        for channel in channels {
                            if let Err(e) = channel.send_message(c, |c| {
                                c.content(format!("New top record from {}!", user_id.mention()))
                                    .embed(|e| score_embed(&score, &beatmap, &user, Some(rank), e))
                            }) {
                                dbg!(e);
                            }
                        }
                    });
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
    fn scan_user(osu: &OsuClient, u: &OsuUser, mode: Mode) -> Result<Vec<(u8, Score)>, Error> {
        let scores = osu.user_best(UserID::ID(u.id), |f| f.mode(mode).limit(25))?;
        let scores = scores
            .into_iter()
            .filter(|s: &Score| s.date >= u.last_update)
            .enumerate()
            .map(|(i, v)| ((i + 1) as u8, v))
            .collect();
        Ok(scores)
    }
}

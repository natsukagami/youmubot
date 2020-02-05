use super::{embeds::score_embed, BeatmapWithMode};
use crate::db::{OsuSavedUsers, OsuUser};
use rayon::prelude::*;
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    http::Http,
    model::id::{ChannelId, UserId},
};
use youmubot_osu::{
    models::{Mode, Score},
    request::{BeatmapRequestKind, UserID},
    Client as Osu,
};
use youmubot_prelude::*;

/// Announce osu! top scores.
pub struct OsuAnnouncer;

impl Announcer for OsuAnnouncer {
    fn announcer_key() -> &'static str {
        "osu"
    }
    fn send_messages(
        c: &Http,
        d: AppData,
        channels: impl Fn(UserId) -> Vec<ChannelId> + Sync,
    ) -> CommandResult {
        let osu = d.get_cloned::<OsuClient>();
        // For each user...
        let mut data = OsuSavedUsers::open(&*d.read()).borrow()?.clone();
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
        *OsuSavedUsers::open(&*d.read()).borrow_mut()? = data;
        Ok(())
    }
}

impl OsuAnnouncer {
    fn scan_user(osu: &Osu, u: &OsuUser, mode: Mode) -> Result<Vec<(u8, Score)>, Error> {
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

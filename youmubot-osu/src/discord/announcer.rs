use super::db::{OsuSavedUsers, OsuUser};
use super::{embeds::score_embed, BeatmapWithMode, OsuClient};
use crate::{
    models::{Mode, Score},
    request::{BeatmapRequestKind, UserID},
    Client as Osu,
};
use announcer::MemberToChannels;
use rayon::prelude::*;
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    http::CacheHttp,
    CacheAndHttp,
};
use std::sync::Arc;
use youmubot_prelude::*;

/// osu! announcer's unique announcer key.
pub const ANNOUNCER_KEY: &'static str = "osu";

/// Announce osu! top scores.
pub fn updates(c: Arc<CacheAndHttp>, d: AppData, channels: MemberToChannels) -> CommandResult {
    let osu = d.get_cloned::<OsuClient>();
    // For each user...
    let mut data = OsuSavedUsers::open(&*d.read()).borrow()?.clone();
    'user_loop: for (user_id, osu_user) in data.iter_mut() {
        let mut pp_values = vec![]; // Store the pp values here...
        for mode in &[Mode::Std, Mode::Taiko, Mode::Mania, Mode::Catch] {
            let scores = scan_user(&osu, osu_user, *mode)?;
            let user = match osu.user(UserID::ID(osu_user.id), |f| f.mode(*mode)) {
                Ok(Some(u)) => u,
                _ => continue 'user_loop,
            };
            pp_values.push(user.pp);
            if scores.is_empty() && !osu_user.pp.is_empty() {
                // Nothing to update: no new scores and pp is there.
                continue;
            }
            scores
                .into_par_iter()
                .filter_map(|(rank, score)| {
                    let beatmap = osu
                        .beatmaps(BeatmapRequestKind::Beatmap(score.beatmap_id), |f| f)
                        .map(|v| BeatmapWithMode(v.into_iter().next().unwrap(), *mode));
                    let channels = channels.channels_of(c.clone(), *user_id);
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
                        if let Err(e) = channel.send_message(c.http(), |c| {
                            c.content(format!("New top record from {}!", user_id.mention()))
                                .embed(|e| score_embed(&score, &beatmap, &user, Some(rank), e))
                        }) {
                            dbg!(e);
                        }
                    }
                });
        }
        osu_user.last_update = chrono::Utc::now();
        osu_user.pp = pp_values;
    }
    // Update users
    *OsuSavedUsers::open(&*d.read()).borrow_mut()? = data;
    Ok(())
}

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

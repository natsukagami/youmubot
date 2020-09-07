use super::db::{OsuSavedUsers, OsuUser};
use super::{embeds::score_embed, BeatmapWithMode, OsuClient};
use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::cache::save_beatmap,
    discord::oppai_cache::BeatmapCache,
    models::{Mode, Score},
    request::UserID,
    Client as Osu,
};
use announcer::MemberToChannels;
use rayon::prelude::*;
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    http::CacheHttp,
    model::id::{ChannelId, UserId},
    CacheAndHttp,
};
use std::sync::Arc;
use youmubot_prelude::*;

/// osu! announcer's unique announcer key.
pub const ANNOUNCER_KEY: &'static str = "osu";

/// The announcer struct implementing youmubot_prelude::Announcer
pub struct Announcer;

#[async_trait]
impl youmubot_prelude::Announcer for Announcer {
    async fn updates(
        &mut self,
        c: Arc<CacheAndHttp>,
        d: AppData,
        channels: MemberToChannels,
    ) -> Result<()> {
        let d = d.read().await;
        let osu = d.get::<OsuClient>().unwrap();
        let cache = d.get::<BeatmapMetaCache>().unwrap();
        let oppai = d.get::<BeatmapCache>().unwrap();
        // For each user...
        let mut data = OsuSavedUsers::open(&*d).borrow()?.clone();
        for (user_id, osu_user) in data.iter_mut() {
            let channels = channels.channels_of(c.clone(), *user_id).await;
            if channels.is_empty() {
                continue; // We don't wanna update an user without any active server
            }
            osu_user.pp = match (&[Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania])
                .par_iter()
                .map(|m| {
                    handle_user_mode(
                        c.clone(),
                        &osu,
                        &cache,
                        &oppai,
                        &osu_user,
                        *user_id,
                        &channels[..],
                        *m,
                        &*d,
                    )
                })
                .collect::<Result<_, _>>()
            {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("osu: Cannot update {}: {}", osu_user.id, e.0);
                    continue;
                }
            };
            osu_user.last_update = chrono::Utc::now();
        }
        // Update users
        *OsuSavedUsers::open(&*d.read()).borrow_mut()? = data;
        Ok(())
    }
}

/// Handles an user/mode scan, announces all possible new scores, return the new pp value.
async fn handle_user_mode(
    c: Arc<CacheAndHttp>,
    osu: &Osu,
    cache: &BeatmapMetaCache,
    oppai: &BeatmapCache,
    osu_user: &OsuUser,
    user_id: UserId,
    channels: &[ChannelId],
    mode: Mode,
    d: &TypeMap,
) -> Result<Option<f64>, Error> {
    let scores = scan_user(osu, osu_user, mode)?;
    let user = osu
        .user(UserID::ID(osu_user.id), |f| f.mode(mode))
        .await?
        .ok_or(Error::from("user not found"))?;
    scores
        .into_iter()
        .map(|(rank, score)| -> Result<_> {
            let beatmap = cache.get_beatmap_default(score.beatmap_id)?;
            let content = oppai.get_beatmap(beatmap.beatmap_id)?;
            Ok((rank, score, BeatmapWithMode(beatmap, mode), content))
        })
        .filter_map(|v| v.ok())
        .for_each(|(rank, score, beatmap, content)| {
            for channel in (&channels).iter() {
                if let Err(e) = channel.send_message(c.http(), |c| {
                    c.content(format!("New top record from {}!", user_id.mention()))
                        .embed(|e| score_embed(&score, &beatmap, &content, &user, Some(rank), e))
                }) {
                    dbg!(e);
                }
                save_beatmap(&*d.read(), *channel, &beatmap).ok();
            }
        });
    Ok(user.pp)
}

fn scan_user(osu: &Osu, u: &OsuUser, mode: Mode) -> Result<Vec<(u8, Score)>, Error> {
    let scores = osu.user_best(UserID::ID(u.id), |f| f.mode(mode).limit(25))?;
    let scores = scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| s.date >= u.last_update)
        .map(|(i, v)| ((i + 1) as u8, v))
        .collect();
    Ok(scores)
}

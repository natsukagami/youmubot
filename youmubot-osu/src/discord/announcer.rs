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
use serenity::{
    http::CacheHttp,
    model::id::{ChannelId, UserId},
    CacheAndHttp,
};
use std::{collections::HashMap, sync::Arc};
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
        // For each user...
        let data = OsuSavedUsers::open(&*d.read().await).borrow()?.clone();
        let data = data
            .into_iter()
            .map(|(user_id, osu_user)| {
                let d = d.clone();
                let channels = &channels;
                let c = c.clone();
                async move {
                    let channels = channels.channels_of(c.clone(), user_id).await;
                    if channels.is_empty() {
                        return (user_id, osu_user); // We don't wanna update an user without any active server
                    }
                    let pp = match (&[Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania])
                        .into_iter()
                        .map(|m| {
                            handle_user_mode(
                                c.clone(),
                                &osu_user,
                                user_id,
                                channels.clone(),
                                *m,
                                d.clone(),
                            )
                        })
                        .collect::<stream::FuturesOrdered<_>>()
                        .try_collect::<Vec<_>>()
                        .await
                    {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("osu: Cannot update {}: {}", osu_user.id, e);
                            return (user_id, osu_user);
                        }
                    };
                    let last_update = chrono::Utc::now();
                    (
                        user_id,
                        OsuUser {
                            pp,
                            last_update,
                            ..osu_user
                        },
                    )
                }
            })
            .collect::<stream::FuturesUnordered<_>>()
            .collect::<HashMap<_, _>>()
            .await;
        // Update users
        *OsuSavedUsers::open(&*d.read().await).borrow_mut()? = data;
        Ok(())
    }
}

/// Handles an user/mode scan, announces all possible new scores, return the new pp value.
async fn handle_user_mode(
    c: Arc<CacheAndHttp>,
    osu_user: &OsuUser,
    user_id: UserId,
    channels: Vec<ChannelId>,
    mode: Mode,
    d: AppData,
) -> Result<Option<f64>, Error> {
    let (scores, user) = {
        let data = d.read().await;
        let osu = data.get::<OsuClient>().unwrap();
        let scores = scan_user(osu, osu_user, mode).await?;
        let user = osu
            .user(UserID::ID(osu_user.id), |f| f.mode(mode))
            .await?
            .ok_or(Error::msg("user not found"))?;
        (scores, user)
    };
    let pp = user.pp;
    spawn_future(async move {
        scores
            .into_iter()
            .map(|(rank, score)| {
                let d = d.clone();
                async move {
                    let data = d.read().await;
                    let cache = data.get::<BeatmapMetaCache>().unwrap();
                    let oppai = data.get::<BeatmapCache>().unwrap();
                    let beatmap = cache.get_beatmap_default(score.beatmap_id).await?;
                    let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
                    let r: Result<_> = Ok((rank, score, BeatmapWithMode(beatmap, mode), content));
                    r
                }
            })
            .collect::<stream::FuturesOrdered<_>>()
            .filter_map(|v| future::ready(v.ok()))
            .for_each(move |(rank, score, beatmap, content)| {
                let channels = channels.clone();
                let d = d.clone();
                let c = c.clone();
                let user = user.clone();
                async move {
                    let data = d.read().await;
                    for channel in (&channels).iter() {
                        if let Err(e) = channel
                            .send_message(c.http(), |c| {
                                c.content(format!("New top record from {}!", user_id.mention()))
                                    .embed(|e| {
                                        score_embed(
                                            &score,
                                            &beatmap,
                                            &content,
                                            &user,
                                            Some(rank),
                                            e,
                                        )
                                    })
                            })
                            .await
                        {
                            dbg!(e);
                        }
                        save_beatmap(&*data, *channel, &beatmap).ok();
                    }
                }
            })
            .await;
    });
    Ok(pp)
}

async fn scan_user(osu: &Osu, u: &OsuUser, mode: Mode) -> Result<Vec<(u8, Score)>, Error> {
    let scores = osu
        .user_best(UserID::ID(u.id), |f| f.mode(mode).limit(25))
        .await?;
    let scores = scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| s.date >= u.last_update)
        .map(|(i, v)| ((i + 1) as u8, v))
        .collect();
    Ok(scores)
}

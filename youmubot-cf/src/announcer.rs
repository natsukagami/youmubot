use crate::{
    db::{CfSavedUsers, CfUser},
    CFClient,
};
use announcer::MemberToChannels;
use chrono::Utc;
use codeforces::{RatingChange, User};
use serenity::{http::CacheHttp, model::id::UserId, CacheAndHttp};
use std::sync::Arc;
use youmubot_prelude::*;

/// Updates the rating and rating changes of the users.
pub struct Announcer;

#[async_trait]
impl youmubot_prelude::Announcer for Announcer {
    async fn updates(
        &mut self,
        http: Arc<CacheAndHttp>,
        data: AppData,
        channels: MemberToChannels,
    ) -> Result<()> {
        let data = data.read().await;
        let client = data.get::<CFClient>().unwrap();
        let mut users = CfSavedUsers::open(&*data).borrow()?.clone();

        users
            .iter_mut()
            .map(|(user_id, cfu)| update_user(http.clone(), &channels, &client, *user_id, cfu))
            .collect::<stream::FuturesUnordered<_>>()
            .try_collect::<()>()
            .await?;
        *CfSavedUsers::open(&*data).borrow_mut()? = users;
        Ok(())
    }
}

async fn update_user(
    http: Arc<CacheAndHttp>,
    channels: &MemberToChannels,
    client: &codeforces::Client,
    user_id: UserId,
    cfu: &mut CfUser,
) -> Result<()> {
    let info = User::info(client, &[cfu.handle.as_str()])
        .await?
        .into_iter()
        .next()
        .ok_or(Error::msg("Not found"))?;

    let rating_changes = info.rating_changes(client).await?;

    let channels_list = channels.channels_of(&http, user_id).await;
    cfu.last_update = Utc::now();
    // Update the rating
    cfu.rating = info.rating;

    let rating_changes = match cfu.last_contest_id {
        None => rating_changes,
        Some(v) => {
            let mut v: Vec<_> = rating_changes
                .into_iter()
                // Skip instead of take because sometimes Codeforces
                // performs rollback.
                .skip_while(|rc| rc.contest_id != v)
                .skip(1)
                .collect();
            v.reverse();
            v
        }
    };

    cfu.last_contest_id = rating_changes
        .first()
        .map(|v| v.contest_id)
        .or(cfu.last_contest_id);

    // Check for any good announcements to make
    rating_changes
        .into_iter()
        .map(|rc: RatingChange| {
            let channels = channels_list.clone();
            let http = http.clone();
            let info = info.clone();
            async move {
                if channels.is_empty() {
                    return Ok(());
                }
                let (contest, _, _) =
                    codeforces::Contest::standings(client, rc.contest_id, |f| f.limit(1, 1))
                        .await?;
                channels
                    .iter()
                    .map(|channel| {
                        channel.send_message(http.http(), |e| {
                            e.content(format!("Rating change for {}!", user_id.mention()))
                                .embed(|c| {
                                    crate::embed::rating_change_embed(
                                        &rc, &info, &contest, user_id, c,
                                    )
                                })
                        })
                    })
                    .collect::<stream::FuturesUnordered<_>>()
                    .map(|v| v.map(|_| ()))
                    .try_collect::<()>()
                    .await?;
                let r: Result<_> = Ok(());
                r
            }
        })
        .collect::<stream::FuturesUnordered<_>>()
        .try_collect::<()>()
        .await?;

    Ok(())
}

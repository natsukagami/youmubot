use std::borrow::Cow;

use chrono::{DateTime, Utc};
use youmubot_db_sql::{models::osu as models, models::osu_user as model, Pool};

use crate::models::{Beatmap, Mode, Score};
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, UserId};
use youmubot_prelude::*;

/// Save the user IDs.
pub struct OsuSavedUsers {
    pool: Pool,
}

impl TypeMapKey for OsuSavedUsers {
    type Value = OsuSavedUsers;
}

impl OsuSavedUsers {
    /// Create a new database wrapper.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

impl OsuSavedUsers {
    /// Get all users
    pub async fn all(&self) -> Result<Vec<OsuUser>> {
        let mut conn = self.pool.acquire().await?;
        model::OsuUser::all(&mut *conn)
            .map(|v| v.map(OsuUser::from).map_err(Error::from))
            .try_collect()
            .await
    }

    /// Get an user by their user_id.
    pub async fn by_user_id(&self, user_id: UserId) -> Result<Option<OsuUser>> {
        let mut conn = self.pool.acquire().await?;
        let u = model::OsuUser::by_user_id(user_id.0 as i64, &mut *conn)
            .await?
            .map(OsuUser::from);
        Ok(u)
    }

    /// Save the given user.
    pub async fn save(&self, u: OsuUser) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        Ok(model::OsuUser::from(u).store(&mut *conn).await?)
    }

    /// Save the given user as a completely new user.
    pub async fn new_user(&self, u: OsuUser) -> Result<()> {
        let mut t = self.pool.begin().await?;
        model::OsuUser::delete(u.user_id.0 as i64, &mut *t).await?;
        model::OsuUser::from(u).store(&mut *t).await?;
        t.commit().await?;
        Ok(())
    }
}

/// Save each channel's last requested beatmap.
pub struct OsuLastBeatmap(Pool);

impl TypeMapKey for OsuLastBeatmap {
    type Value = OsuLastBeatmap;
}

impl OsuLastBeatmap {
    pub fn new(pool: Pool) -> Self {
        Self(pool)
    }
}

impl OsuLastBeatmap {
    pub async fn by_channel(&self, id: impl Into<ChannelId>) -> Result<Option<(Beatmap, Mode)>> {
        let last_beatmap = models::LastBeatmap::by_channel_id(id.into().0 as i64, &self.0).await?;
        Ok(match last_beatmap {
            Some(lb) => Some((bincode::deserialize(&lb.beatmap[..])?, lb.mode.into())),
            None => None,
        })
    }

    pub async fn save(
        &self,
        channel: impl Into<ChannelId>,
        beatmap: &Beatmap,
        mode: Mode,
    ) -> Result<()> {
        let b = models::LastBeatmap {
            channel_id: channel.into().0 as i64,
            beatmap: bincode::serialize(beatmap)?,
            mode: mode as u8,
        };
        b.store(&self.0).await?;
        Ok(())
    }
}

/// Save each channel's last requested beatmap.
pub struct OsuUserBests(Pool);

impl TypeMapKey for OsuUserBests {
    type Value = OsuUserBests;
}

impl OsuUserBests {
    pub fn new(pool: Pool) -> Self {
        Self(pool)
    }
}

impl OsuUserBests {
    pub async fn save(
        &self,
        user: impl Into<UserId>,
        mode: Mode,
        scores: impl IntoIterator<Item = Score>,
    ) -> Result<()> {
        let user = user.into();
        scores
            .into_iter()
            .map(|score| models::UserBestScore {
                user_id: user.0 as i64,
                beatmap_id: score.beatmap_id as i64,
                mode: mode as u8,
                mods: score.mods.bits() as i64,
                cached_at: Utc::now(),
                score: bincode::serialize(&score).unwrap(),
            })
            .map(|mut us| async move { us.store(&self.0).await })
            .collect::<stream::FuturesUnordered<_>>()
            .try_collect::<()>()
            .await?;
        Ok(())
    }
}

/// An osu! saved user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUser {
    pub user_id: UserId,
    pub username: Cow<'static, str>,
    pub id: u64,
    pub last_update: DateTime<Utc>,
    pub pp: [Option<f64>; 4],
    /// More than 5 failures => gone
    pub failures: u8,
}

impl From<OsuUser> for model::OsuUser {
    fn from(u: OsuUser) -> Self {
        Self {
            user_id: u.user_id.0 as i64,
            username: Some(u.username.into_owned()),
            id: u.id as i64,
            last_update: u.last_update,
            pp_std: u.pp[Mode::Std as usize],
            pp_taiko: u.pp[Mode::Taiko as usize],
            pp_catch: u.pp[Mode::Catch as usize],
            pp_mania: u.pp[Mode::Mania as usize],
            failures: u.failures,
        }
    }
}

impl From<model::OsuUser> for OsuUser {
    fn from(u: model::OsuUser) -> Self {
        Self {
            user_id: UserId(u.user_id as u64),
            username: u.username.map(Cow::Owned).unwrap_or("unknown".into()),
            id: u.id as u64,
            last_update: u.last_update,
            pp: [0, 1, 2, 3].map(|v| match Mode::from(v) {
                Mode::Std => u.pp_std,
                Mode::Taiko => u.pp_taiko,
                Mode::Catch => u.pp_catch,
                Mode::Mania => u.pp_mania,
            }),
            failures: u.failures,
        }
    }
}

#[allow(dead_code)]
mod legacy {
    use chrono::{DateTime, Utc};

    use crate::models::{Beatmap, Mode, Score};
    use serde::{Deserialize, Serialize};
    use serenity::model::id::{ChannelId, UserId};
    use std::collections::HashMap;
    use youmubot_db::DB;

    pub type OsuSavedUsers = DB<HashMap<UserId, OsuUser>>;

    /// An osu! saved user.
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct OsuUser {
        pub id: u64,
        pub last_update: DateTime<Utc>,
        #[serde(default)]
        pub pp: Vec<Option<f64>>,
        /// More than 5 failures => gone
        pub failures: Option<u8>,
    }

    /// Save each channel's last requested beatmap.
    pub type OsuLastBeatmap = DB<HashMap<ChannelId, (Beatmap, Mode)>>;

    /// Save each beatmap's plays by user.
    pub type OsuUserBests =
        DB<HashMap<(u64, Mode) /* Beatmap ID and Mode */, HashMap<UserId, Vec<Score>>>>;
}

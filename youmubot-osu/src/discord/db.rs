use chrono::{DateTime, Utc};
use youmubot_db_sql::{models::osu as models, models::osu_user as model, Pool};

use crate::models::{Beatmap, Mode, Score};
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, UserId};
use std::collections::HashMap;
use youmubot_db::DB;
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
        model::OsuUser::all(&mut conn)
            .map(|v| v.map(OsuUser::from).map_err(Error::from))
            .try_collect()
            .await
    }

    /// Get an user by their user_id.
    pub async fn by_user_id(&self, user_id: UserId) -> Result<Option<OsuUser>> {
        let mut conn = self.pool.acquire().await?;
        let u = model::OsuUser::by_user_id(user_id.0 as i64, &mut conn)
            .await?
            .map(OsuUser::from);
        Ok(u)
    }

    /// Save the given user.
    pub async fn save(&self, u: OsuUser) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        Ok(model::OsuUser::from(u).store(&mut conn).await?)
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

/// Save each beatmap's plays by user.
pub type OsuUserBests =
    DB<HashMap<(u64, Mode) /* Beatmap ID and Mode */, HashMap<UserId, Vec<Score>>>>;

/// An osu! saved user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUser {
    pub user_id: UserId,
    pub id: u64,
    pub last_update: DateTime<Utc>,
    pub pp: [Option<f32>; 4],
    /// More than 5 failures => gone
    pub failures: u8,
}

impl From<OsuUser> for model::OsuUser {
    fn from(u: OsuUser) -> Self {
        Self {
            user_id: u.user_id.0 as i64,
            id: u.id as i64,
            last_update: u.last_update,
            pp_std: u.pp[Mode::Std as usize],
            pp_taiko: u.pp[Mode::Taiko as usize],
            pp_mania: u.pp[Mode::Mania as usize],
            pp_catch: u.pp[Mode::Catch as usize],
            failures: u.failures,
        }
    }
}

impl From<model::OsuUser> for OsuUser {
    fn from(u: model::OsuUser) -> Self {
        Self {
            user_id: UserId(u.user_id as u64),
            id: u.id as u64,
            last_update: u.last_update,
            pp: [u.pp_std, u.pp_taiko, u.pp_mania, u.pp_catch],
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

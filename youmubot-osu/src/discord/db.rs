use std::borrow::Cow;
use std::collections::HashMap as Map;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, UserId};

use youmubot_db_sql::{models::osu as models, models::osu_user as model, Pool};
use youmubot_prelude::*;

use crate::models::{Beatmap, Mode};

/// Save the user IDs.
#[derive(Debug, Clone)]
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
        Ok(model::OsuUser::all(&self.pool)
            .await?
            .into_iter()
            .map(|v| v.into())
            .collect())
    }

    /// Get an user by their user_id.
    pub async fn by_user_id(&self, user_id: UserId) -> Result<Option<OsuUser>> {
        let u = model::OsuUser::by_user_id(user_id.get() as i64, &self.pool)
            .await?
            .map(OsuUser::from);
        Ok(u)
    }

    /// Save the given user.
    pub async fn save(&self, u: OsuUser) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let updated = model::OsuUser::from(u).store(&mut tx).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Save the given user as a completely new user.
    pub async fn new_user(&self, u: OsuUser) -> Result<()> {
        let mut t = self.pool.begin().await?;
        model::OsuUser::delete(u.user_id.get() as i64, &mut *t).await?;
        assert!(
            model::OsuUser::from(u).store(&mut t).await?,
            "Should be updated"
        );
        t.commit().await?;
        Ok(())
    }
}

/// Save each channel's last requested beatmap.
#[derive(Debug, Clone)]
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
        let last_beatmap =
            models::LastBeatmap::by_channel_id(id.into().get() as i64, &self.0).await?;
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
            channel_id: channel.into().get() as i64,
            beatmap: bincode::serialize(beatmap)?,
            mode: mode as u8,
        };
        b.store(&self.0).await?;
        Ok(())
    }
}

/// An osu! saved user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUser {
    pub user_id: UserId,
    pub username: Cow<'static, str>,
    pub id: u64,
    pub modes: Map<Mode, OsuUserMode>,
    /// More than 5 failures => gone
    pub failures: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUserMode {
    pub pp: f64,
    pub map_length: f64,
    pub map_age: i64,
    pub last_update: DateTime<Utc>,
}

impl From<OsuUser> for model::OsuUser {
    fn from(u: OsuUser) -> Self {
        Self {
            user_id: u.user_id.get() as i64,
            username: Some(u.username.into_owned()),
            id: u.id as i64,
            modes: u
                .modes
                .into_iter()
                .map(|(k, v)| (k as u8, v.into()))
                .collect(),
            failures: u.failures,
        }
    }
}

impl From<model::OsuUser> for OsuUser {
    fn from(u: model::OsuUser) -> Self {
        Self {
            user_id: UserId::new(u.user_id as u64),
            username: u.username.map(Cow::Owned).unwrap_or("unknown".into()),
            id: u.id as u64,
            modes: u
                .modes
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            failures: u.failures,
        }
    }
}

impl From<OsuUserMode> for model::OsuUserMode {
    fn from(m: OsuUserMode) -> Self {
        Self {
            pp: m.pp,
            map_length: m.map_length,
            map_age: m.map_age,
            last_update: m.last_update,
        }
    }
}

impl From<model::OsuUserMode> for OsuUserMode {
    fn from(m: model::OsuUserMode) -> Self {
        Self {
            pp: m.pp,
            map_length: m.map_length,
            map_age: m.map_age,
            last_update: m.last_update,
        }
    }
}

#[allow(dead_code)]
mod legacy {
    use std::collections::HashMap;

    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use serenity::model::id::{ChannelId, UserId};

    use youmubot_db::DB;

    use crate::models::{Beatmap, Mode, Score};

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

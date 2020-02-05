use chrono::{DateTime, Utc};
use dotenv::var;

use serde::{Deserialize, Serialize};
use serenity::{
    client::Client,
    framework::standard::CommandError as Error,
    model::id::{ChannelId, RoleId, UserId},
};
use std::collections::HashMap;
use std::path::PathBuf;
use youmubot_db::{GuildMap, DB};
use youmubot_osu::models::{Beatmap, Mode};

/// A list of SoftBans for all servers.
pub type SoftBans = DB<GuildMap<ServerSoftBans>>;

/// Save the user IDs.
pub type OsuSavedUsers = DB<HashMap<UserId, OsuUser>>;

/// Save each channel's last requested beatmap.
pub type OsuLastBeatmap = DB<HashMap<ChannelId, (Beatmap, Mode)>>;

/// Sets up all databases in the client.
pub fn setup_db(client: &mut Client) -> Result<(), Error> {
    let path: PathBuf = var("DBPATH").map(|v| PathBuf::from(v)).unwrap_or_else(|e| {
        println!("No DBPATH set up ({:?}), using `/data`", e);
        PathBuf::from("data")
    });
    let mut data = client.data.write();
    SoftBans::insert_into(&mut *data, &path.join("soft_bans.yaml"))?;
    OsuSavedUsers::insert_into(&mut *data, &path.join("osu_saved_users.yaml"))?;
    OsuLastBeatmap::insert_into(&mut *data, &path.join("last_beatmaps.yaml"))?;
    // AnnouncerChannels::insert_into(&mut *data, &path.join("announcers.yaml"))?;

    Ok(())
}

/// For the admin commands:
///  - Each server might have a `soft ban` role implemented.
///  - We allow periodical `soft ban` applications.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerSoftBans {
    Implemented(ImplementedSoftBans),
    Unimplemented,
}

impl ServerSoftBans {
    // Create a new, implemented role.
    pub fn new_implemented(role: RoleId) -> ServerSoftBans {
        ServerSoftBans::Implemented(ImplementedSoftBans {
            role,
            periodical_bans: HashMap::new(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImplementedSoftBans {
    /// The soft-ban role.
    pub role: RoleId,
    /// List of all to-unban people.
    pub periodical_bans: HashMap<UserId, DateTime<Utc>>,
}

/// An osu! saved user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUser {
    pub id: u64,
    pub last_update: DateTime<Utc>,
}

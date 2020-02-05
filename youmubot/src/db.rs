use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use serenity::{
    framework::standard::CommandError as Error,
    model::id::{RoleId, UserId},
};
use std::collections::HashMap;
use std::path::Path;
use youmubot_db::{GuildMap, DB};
use youmubot_prelude::*;

/// A list of SoftBans for all servers.
pub type SoftBans = DB<GuildMap<ServerSoftBans>>;

/// Sets up all databases in the client.
pub fn setup_db(path: &Path, data: &mut ShareMap) -> Result<(), Error> {
    SoftBans::insert_into(&mut *data, &path.join("soft_bans.yaml"))?;

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

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use serenity::model::id::{RoleId, UserId};
use std::collections::HashMap;
use youmubot_db::{GuildMap, DB};

/// A list of SoftBans for all servers.
pub type SoftBans = DB<GuildMap<ServerSoftBans>>;

/// A list of assignable roles for all servers.
pub type Roles = DB<GuildMap<HashMap<RoleId, Role>>>;

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

/// Role represents an assignable role.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Role {
    pub id: RoleId,
    pub description: String,
}

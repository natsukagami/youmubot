use serde::{Deserialize, Serialize};
use serenity::model::{
    channel::ReactionType,
    id::{MessageId, RoleId},
};
use std::collections::HashMap;
use youmubot_db::{GuildMap, DB};
use youmubot_prelude::*;

/// A list of assignable roles for all servers.
pub type Roles = DB<GuildMap<RoleList>>;

/// Represents a server's role list.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RoleList {
    /// `reaction_message` handles the reaction-handling message.
    pub reaction_messages: HashMap<MessageId, RoleMessage>,
    pub roles: HashMap<RoleId, Role>,
}

/// Load the file list, handling migration from v1.
pub fn load_role_list(
    map: &mut TypeMap,
    path: impl AsRef<std::path::Path>,
    v1_path: impl AsRef<std::path::Path>,
) -> Result<()> {
    // Try to load v2 first
    let v2 = Roles::load_from_path(path.as_ref());
    let v2 = match v2 {
        Ok(v2) => {
            map.insert::<Roles>(v2);
            return Ok(());
        }
        Err(v2) => v2,
    };
    // Try migrating from v1.
    match legacy::RolesV1::load_from_path(v1_path.as_ref()) {
        Ok(v1) => {
            Roles::insert_into(map, path)?;
            *Roles::open(map).borrow_mut()? = v1
                .get_data(true)?
                .into_iter()
                .map(|(guild, roles)| {
                    (
                        guild,
                        RoleList {
                            reaction_messages: HashMap::new(),
                            roles,
                        },
                    )
                })
                .collect();
            std::fs::remove_file(v1_path.as_ref()).pls_ok();
            eprintln!("Migrated roles v1 to v2.");
            Ok(())
        }
        Err(v1) => Err(Error::msg(format!(
            "failed with v2 ({}) and v1 ({})",
            v2, v1
        ))),
    }
}

/// A single role in the list of role messages.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoleMessage {
    pub id: serenity::model::id::MessageId,
    pub title: String,
    pub roles: Vec<(Role, ReactionType)>,
}

/// Role represents an assignable role.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Role {
    pub id: RoleId,
    pub description: String,
    #[serde(default)]
    pub reaction: Option<ReactionType>,
}

mod legacy {
    use super::Role;
    use serenity::model::id::RoleId;
    use std::collections::HashMap;
    use youmubot_db::{GuildMap, DB};
    /// (Depreciated) A list of assignable roles for all servers.
    pub type RolesV1 = DB<GuildMap<HashMap<RoleId, Role>>>;
}

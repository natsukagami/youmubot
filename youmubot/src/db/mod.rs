use chrono::{DateTime, Utc};
use dotenv::var;
use rustbreak::{deser::Yaml as Ron, FileDatabase};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serenity::{
    client::Client,
    framework::standard::CommandError as Error,
    model::id::{ChannelId, GuildId, RoleId, UserId},
    prelude::*,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use youmubot_osu::models::Mode;

/// GuildMap defines the guild-map type.
/// It is basically a HashMap from a GuildId to a data structure.
pub type GuildMap<V> = HashMap<GuildId, V>;
/// The generic DB type we will be using.
pub struct DB<T>(std::marker::PhantomData<T>);
impl<T: std::any::Any> serenity::prelude::TypeMapKey for DB<T> {
    type Value = FileDatabase<T, Ron>;
}

impl<T: std::any::Any + Default + Send + Sync + Clone + Serialize + std::fmt::Debug> DB<T>
where
    for<'de> T: Deserialize<'de>,
{
    fn insert_into(data: &mut ShareMap, path: impl AsRef<Path>) -> Result<(), Error> {
        let db = FileDatabase::<T, Ron>::from_path(path, T::default())?;
        db.load().or_else(|_| db.save())?;
        data.insert::<DB<T>>(db);
        Ok(())
    }
}

/// A list of SoftBans for all servers.
pub type SoftBans = DB<GuildMap<ServerSoftBans>>;

/// Save the user IDs.
pub type OsuSavedUsers = DB<HashMap<UserId, u64>>;

/// Save each channel's last requested beatmap.
pub type OsuLastBeatmap = DB<HashMap<ChannelId, (u64, Mode)>>;

/// Sets up all databases in the client.
pub fn setup_db(client: &mut Client) -> Result<(), Error> {
    let path: PathBuf = var("DBPATH").map(|v| PathBuf::from(v)).unwrap_or_else(|e| {
        println!("No DBPATH set up ({:?}), using `/data`", e);
        PathBuf::from("data")
    });
    let mut data = client.data.write();
    SoftBans::insert_into(&mut *data, &path.join("soft_bans.ron"))?;
    OsuSavedUsers::insert_into(&mut *data, &path.join("osu_saved_users.ron"))?;
    OsuLastBeatmap::insert_into(&mut *data, &path.join("last_beatmaps.ron"))?;

    Ok(())
}

pub struct DBWriteGuard<'a, T>(&'a mut FileDatabase<T, Ron>)
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned;

impl<'a, T> From<&'a mut FileDatabase<T, Ron>> for DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn from(v: &'a mut FileDatabase<T, Ron>) -> Self {
        DBWriteGuard(v)
    }
}

impl<'a, T> DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    pub fn borrow(&self) -> Result<std::sync::RwLockReadGuard<T>, rustbreak::RustbreakError> {
        (*self).0.borrow_data()
    }
    pub fn borrow_mut(
        &mut self,
    ) -> Result<std::sync::RwLockWriteGuard<T>, rustbreak::RustbreakError> {
        (*self).0.borrow_data_mut()
    }
}

impl<'a, T> Drop for DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn drop(&mut self) {
        self.0.save().expect("Save succeed")
    }
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
            role: role,
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

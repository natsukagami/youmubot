use rustbreak::{deser::Yaml as Ron, FileDatabase};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serenity::{framework::standard::CommandError as Error, model::id::GuildId, prelude::*};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// GuildMap defines the guild-map type.
/// It is basically a HashMap from a GuildId to a data structure.
pub type GuildMap<V> = HashMap<GuildId, V>;
/// The generic DB type we will be using.
pub struct DB<T>(std::marker::PhantomData<T>);
impl<T: std::any::Any> serenity::prelude::TypeMapKey for DB<T> {
    type Value = Arc<FileDatabase<T, Ron>>;
}

impl<T: std::any::Any + Default + Send + Sync + Clone + Serialize + std::fmt::Debug> DB<T>
where
    for<'de> T: Deserialize<'de>,
{
    /// Insert into a ShareMap.
    pub fn insert_into(data: &mut ShareMap, path: impl AsRef<Path>) -> Result<(), Error> {
        let db = FileDatabase::<T, Ron>::from_path(path, T::default())?;
        db.load().or_else(|e| {
            dbg!(e);
            db.save()
        })?;
        data.insert::<DB<T>>(Arc::new(db));
        Ok(())
    }

    /// Open a previously inserted DB.
    pub fn open(data: &ShareMap) -> DBWriteGuard<T> {
        data.get::<Self>().expect("DB initialized").clone().into()
    }
}

/// The write guard for our FileDatabase.
/// It wraps the FileDatabase in a write-on-drop lock.
#[derive(Debug)]
pub struct DBWriteGuard<T>(Arc<FileDatabase<T, Ron>>)
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned;

impl<T> From<Arc<FileDatabase<T, Ron>>> for DBWriteGuard<T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn from(v: Arc<FileDatabase<T, Ron>>) -> Self {
        DBWriteGuard(v)
    }
}

impl<T> DBWriteGuard<T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    /// Borrows the FileDatabase.
    pub fn borrow(&self) -> Result<std::sync::RwLockReadGuard<T>, rustbreak::RustbreakError> {
        (*self).0.borrow_data()
    }
    /// Borrows the FileDatabase for writing.
    pub fn borrow_mut(&self) -> Result<std::sync::RwLockWriteGuard<T>, rustbreak::RustbreakError> {
        (*self).0.borrow_data_mut()
    }
}

impl<T> Drop for DBWriteGuard<T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn drop(&mut self) {
        if let Err(e) = self.0.save() {
            dbg!(e);
        }
    }
}

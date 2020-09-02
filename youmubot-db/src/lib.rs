use rustbreak::{deser::Yaml, FileDatabase};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serenity::{framework::standard::CommandError as Error, model::id::GuildId, prelude::*};
use std::{collections::HashMap, path::Path};

/// GuildMap defines the guild-map type.
/// It is basically a HashMap from a GuildId to a data structure.
pub type GuildMap<V> = HashMap<GuildId, V>;
/// The generic DB type we will be using.
pub struct DB<T>(std::marker::PhantomData<T>);

/// A short type abbreviation for a FileDatabase.
type Database<T> = FileDatabase<T, Yaml>;

impl<T: std::any::Any + Send + Sync> serenity::prelude::TypeMapKey for DB<T> {
    type Value = Database<T>;
}

impl<T: std::any::Any + Default + Send + Sync + Clone + Serialize + std::fmt::Debug> DB<T>
where
    for<'de> T: Deserialize<'de>,
{
    /// Insert into a ShareMap.
    pub fn insert_into(data: &mut TypeMap, path: impl AsRef<Path>) -> Result<(), Error> {
        let db = Database::<T>::load_from_path_or_default(path)?;
        data.insert::<DB<T>>(db);
        Ok(())
    }

    /// Open a previously inserted DB.
    pub fn open(data: &TypeMap) -> DBWriteGuard<T> {
        data.get::<Self>().expect("DB initialized").into()
    }
}

/// The write guard for our FileDatabase.
/// It wraps the FileDatabase in a write-on-drop lock.
#[derive(Debug)]
pub struct DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    db: &'a Database<T>,
    needs_save: bool,
}

impl<'a, T> From<&'a Database<T>> for DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn from(v: &'a Database<T>) -> Self {
        DBWriteGuard {
            db: v,
            needs_save: false,
        }
    }
}

impl<'a, T> DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    /// Borrows the FileDatabase.
    pub fn borrow(&'a self) -> Result<std::sync::RwLockReadGuard<T>, rustbreak::RustbreakError> {
        self.db.borrow_data()
    }
    /// Borrows the FileDatabase for writing.
    pub fn borrow_mut(
        &'a mut self,
    ) -> Result<std::sync::RwLockWriteGuard<T>, rustbreak::RustbreakError> {
        self.needs_save = true;
        self.db.borrow_data_mut()
    }
}

impl<'a, T> Drop for DBWriteGuard<'a, T>
where
    T: Send + Sync + Clone + std::fmt::Debug + Serialize + DeserializeOwned,
{
    fn drop(&mut self) {
        if self.needs_save {
            if let Err(e) = self.db.save() {
                dbg!(e);
            }
        }
    }
}

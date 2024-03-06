use anyhow::bail;
use chrono::{DateTime, Utc};
use serenity::model::{
    guild::Member,
    id::{GuildId, UserId},
};
use serenity::{http::CacheHttp, prelude::*};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

const VALID_CACHE_SECONDS: i64 = 15 * 60; // 15 minutes
const INVALID_CACHE_SECONDS: i64 = 2 * 60; // 2 minutes

type Map<K, V> = Mutex<HashMap<K, V>>;

/// MemberCache resolves `does User belong to Guild` requests, and store them in a cache.
#[derive(Debug, Default)]
pub struct MemberCache {
    per_user: Map<(UserId, GuildId), Expiring<Option<Member>>>,
    per_guild: Map<GuildId, Expiring<Option<Arc<[Member]>>>>,
}

#[derive(Debug)]
struct Expiring<T> {
    pub value: T,
    pub timeout: DateTime<Utc>,
}

impl<T> Expiring<T> {
    fn new(value: T, timeout: DateTime<Utc>) -> Self {
        Self { value, timeout }
    }
}

impl<T> std::ops::Deref for Expiring<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl TypeMapKey for MemberCache {
    type Value = Arc<MemberCache>;
}

impl MemberCache {
    pub async fn query(
        &self,
        cache_http: impl CacheHttp,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Option<Member> {
        let now = Utc::now();
        // Check cache
        if let Some(r) = self.per_user.lock().await.get(&(user_id, guild_id)) {
            if r.timeout > now {
                return r.value.clone();
            }
        }
        // Check members cache first if possible
        if let Ok(mems) = self.query_members(&cache_http, guild_id).await {
            return mems.iter().find(|m| m.user.id == user_id).cloned();
        }
        // Query
        let mut map = self.per_user.lock().await;
        let entry = map.entry((user_id, guild_id));
        if let Entry::Occupied(oe) = &entry {
            if oe.get().timeout > now {
                return oe.get().value.clone();
            }
        }
        let t = guild_id.member(&cache_http, user_id).await.ok();
        entry
            .or_insert(Expiring::new(
                t.clone(),
                now + chrono::Duration::seconds(VALID_CACHE_SECONDS),
            ))
            .value
            .clone()
    }

    pub async fn query_members(
        &self,
        cache_http: impl CacheHttp,
        guild_id: GuildId,
    ) -> crate::Result<Arc<[Member]>> {
        let now = Utc::now();
        let mut map = self.per_guild.lock().await;
        let entry = map.entry(guild_id);
        // Check cache
        if let Entry::Occupied(oe) = &entry {
            if oe.get().timeout > now {
                return match &oe.get().value {
                    Some(v) => Ok(v.clone()),
                    None => bail!("guild members for {} unavailable", guild_id),
                };
            }
        }
        // query
        eprintln!("querying members of {}", guild_id);
        let members: Option<Arc<[Member]>> = guild_id
            .members(cache_http.http(), None, None)
            .await
            .ok()
            .map(|v| v.into());
        match &entry
            .or_insert(Expiring::new(
                members.clone(),
                now + chrono::Duration::seconds(if members.is_some() {
                    VALID_CACHE_SECONDS
                } else {
                    INVALID_CACHE_SECONDS
                }),
            ))
            .value
        {
            Some(v) => Ok(v.clone()),
            None => bail!("guild members for {} unavailable", guild_id),
        }
    }
}

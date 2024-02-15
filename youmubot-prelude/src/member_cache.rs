use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serenity::model::{
    guild::Member,
    id::{GuildId, UserId},
};
use serenity::{http::CacheHttp, prelude::*};
use std::sync::Arc;

const VALID_CACHE_SECONDS: i64 = 15 * 60; // 15 minutes

/// MemberCache resolves `does User belong to Guild` requests, and store them in a cache.
#[derive(Debug, Default)]
pub struct MemberCache {
    per_user: DashMap<(UserId, GuildId), Expiring<Option<Member>>>,
    per_guild: DashMap<GuildId, Expiring<Arc<[Member]>>>,
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
        if let Some(r) = self.per_user.get(&(user_id, guild_id)) {
            if r.timeout > now {
                return r.clone();
            }
        }
        // Query
        let t = guild_id.member(&cache_http, user_id).await.ok();
        self.per_user.insert(
            (user_id, guild_id),
            Expiring::new(
                t.clone(),
                now + chrono::Duration::seconds(VALID_CACHE_SECONDS),
            ),
        );
        t
    }

    pub async fn query_members(
        &self,
        cache_http: impl CacheHttp,
        guild_id: GuildId,
    ) -> crate::Result<Arc<[Member]>> {
        let now = Utc::now();
        // Check cache
        if let Some(r) = self.per_guild.get(&guild_id) {
            if r.timeout > now {
                return Ok(r.value.clone());
            }
        }
        // query
        let members: Arc<[Member]> = guild_id
            .members(cache_http.http(), None, None)
            .await?
            .into();
        self.per_guild.insert(
            guild_id,
            Expiring::new(
                members.clone(),
                now + chrono::Duration::seconds(VALID_CACHE_SECONDS),
            ),
        );
        Ok(members)
    }
}

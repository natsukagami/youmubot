use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serenity::model::{
    guild::Member,
    id::{GuildId, UserId},
};
use serenity::{http::CacheHttp, prelude::*};
use std::collections::HashMap as Map;
use std::sync::Arc;

use crate::OkPrint;

const VALID_CACHE_SECONDS: i64 = 15 * 60; // 15 minutes

/// MemberCache resolves `does User belong to Guild` requests, and store them in a cache.
#[derive(Debug, Default)]
pub struct MemberCache {
    per_user: DashMap<(UserId, GuildId), Expiring<Option<Member>>>,
    per_guild: DashMap<GuildId, Expiring<Map<UserId, Member>>>,
    guild_counts: DashMap<GuildId, Option<u64>>,
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
        let members_count = match self.guild_counts.get(&guild_id) {
            Some(v) => v.clone(),
            None => {
                let res = guild_id
                    .to_partial_guild_with_counts(cache_http.http())
                    .await
                    .ok()
                    .and_then(|v| v.approximate_member_count);
                self.guild_counts.insert(guild_id, res);
                res
            }
        };
        match members_count {
            Some(g) if g <= 1000 => self.query_per_guild(cache_http, user_id, guild_id).await,
            _ => self.query_per_user(cache_http, user_id, guild_id).await,
        }
    }

    async fn query_per_guild(
        &self,
        cache_http: impl CacheHttp,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Option<Member> {
        let now = Utc::now();
        // Check cache
        if let Some(r) = self.per_guild.get(&guild_id) {
            if r.timeout > now {
                return r.get(&user_id).cloned();
            }
        }
        // query
        let members = guild_id
            .members(cache_http.http(), None, None)
            .await
            .pls_ok()?
            .into_iter()
            .map(|m| (m.user.id, m))
            .collect::<Map<_, _>>();
        let result = members.get(&user_id).cloned();
        self.per_guild.insert(
            guild_id,
            Expiring::new(
                members,
                now + chrono::Duration::seconds(VALID_CACHE_SECONDS),
            ),
        );
        result
    }

    async fn query_per_user(
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
}

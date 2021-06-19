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
pub struct MemberCache(DashMap<(UserId, GuildId), (Option<Member>, DateTime<Utc>)>);

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
        if let Some(r) = self.0.get(&(user_id, guild_id)) {
            if r.1 > now {
                return r.0.clone();
            }
        }
        // Query
        let t = guild_id.member(&cache_http, user_id).await.ok();
        self.0.insert(
            (user_id, guild_id),
            (
                t.clone(),
                now + chrono::Duration::seconds(VALID_CACHE_SECONDS),
            ),
        );
        t
    }
}

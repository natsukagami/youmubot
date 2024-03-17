use std::{path::Path, time::Duration};

use serenity::prelude::*;

use crate::Env;

/// Set up the prelude libraries.
///
/// Panics on failure: Youmubot should *NOT* attempt to continue when this function fails.
pub async fn setup_prelude(
    db_path: impl AsRef<Path>,
    sql_path: impl AsRef<Path>,
    data: &mut TypeMap,
) -> Env {
    // Set up the announcer DB.
    crate::announcer::AnnouncerChannels::insert_into(
        data,
        db_path.as_ref().join("announcers.yaml"),
    )
    .expect("Announcers DB set up");

    // Set up the database
    let sql_pool = youmubot_db_sql::connect(sql_path)
        .await
        .expect("SQL database set up");

    // Set up the HTTP client.
    let http_client = reqwest::ClientBuilder::new()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(60))
        .build()
        .expect("Build be able to build HTTP client");
    data.insert::<crate::HTTPClient>(http_client.clone());

    // Set up the member cache.
    let member_cache = std::sync::Arc::new(crate::MemberCache::default());
    data.insert::<crate::MemberCache>(member_cache.clone());

    // Set up the SQL client.
    data.insert::<crate::SQLClient>(sql_pool.clone());

    let env = Env {
        http: http_client,
        sql: sql_pool,
        members: member_cache,
    };

    env
}

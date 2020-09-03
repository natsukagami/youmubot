use serenity::{framework::standard::StandardFramework, prelude::*};
use std::path::Path;

/// Set up the prelude libraries.
///
/// Panics on failure: Youmubot should *NOT* attempt to continue when this function fails.
pub fn setup_prelude(db_path: &Path, data: &mut TypeMap, _: &mut StandardFramework) {
    // Setup the announcer DB.
    crate::announcer::AnnouncerChannels::insert_into(data, db_path.join("announcers.yaml"))
        .expect("Announcers DB set up");

    // Set up the HTTP client.
    data.insert::<crate::HTTPClient>(reqwest::Client::new());
}

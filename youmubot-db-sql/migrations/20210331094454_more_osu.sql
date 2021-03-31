-- Add migration script here

CREATE TABLE osu_last_beatmaps (
    channel_id    BIGINT NOT NULL PRIMARY KEY,
    beatmap_id    BIGINT NOT NULL,
    beatmapset_id BIGINT NOT NULL,
    mode          INT    NOT NULL
);

CREATE TABLE osu_user_best_scores (
    beatmap_id BIGINT NOT NULL,
    mode       INT    NOT NULL,
    user_id    INT    NOT NULL,
    mods       INT    NOT NULL,

    cached_at DATETIME NOT NULL,
    score     BLOB     NOT NULL,

    PRIMARY KEY (beatmap_id, mode, user_id, mods)
);

CREATE TABLE osu_cached_beatmaps (
    beatmap_id BIGINT NOT NULL,
    mode       INT NOT NULL,
    
    cached_at DATETIME NOT NULL,
    beatmap   BLOB     NOT NULL,

    PRIMARY KEY (beatmap_id, mode)
);

CREATE TABLE osu_cached_beatmap_contents (
    beatmap_id BIGINT NOT NULL PRIMARY KEY,

    cached_at DATETIME NOT NULL,
    content   BLOB     NOT NULL
);

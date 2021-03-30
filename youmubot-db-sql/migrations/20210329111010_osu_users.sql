-- Add migration script here

CREATE TABLE osu_users (
    user_id     BIGINT   NOT NULL PRIMARY KEY,
    id          BIGINT   NOT NULL UNIQUE,
    last_update DATETIME NULL,
    pp_std      REAL     NULL,
    pp_taiko    REAL     NULL,
    pp_mania    REAL     NULL,
    pp_catch    REAL     NULL,
    failures    INT      NOT NULL DEFAULT 0
);

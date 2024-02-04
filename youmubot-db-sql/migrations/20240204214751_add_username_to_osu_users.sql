-- Add migration script here

ALTER TABLE osu_users
    ADD COLUMN username TEXT NULL DEFAULT NULL;

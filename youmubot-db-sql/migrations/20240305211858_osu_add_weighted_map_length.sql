-- Add migration script here

ALTER TABLE osu_users
    ADD COLUMN std_weighted_map_length DOUBLE NULL DEFAULT NULL;


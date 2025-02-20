-- Add migration script here

ALTER TABLE osu_last_beatmaps RENAME COLUMN mode TO mode_old;
ALTER TABLE osu_last_beatmaps ADD COLUMN mode INT NULL;
UPDATE osu_last_beatmaps SET mode = mode_old;
ALTER TABLE osu_last_beatmaps DROP COLUMN mode_old;

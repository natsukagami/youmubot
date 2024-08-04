-- Add migration script here

CREATE TABLE osu_user_mode_stats (
  user_id INT NOT NULL REFERENCES osu_users (user_id) ON DELETE CASCADE,
  mode INT NOT NULL,
  pp REAL NOT NULL DEFAULT 0,
  map_length REAL NOT NULL DEFAULT 0,
  map_age INT NOT NULL DEFAULT 0,
  last_update INT NOT NULL,
  PRIMARY KEY (user_id, mode),
  CHECK (mode >= 0 AND mode < 4)
) STRICT;

-- Try to move data to new table

INSERT INTO osu_user_mode_stats (user_id, mode, pp, map_length, last_update)
  SELECT 
    u.user_id,
    0 as mode,
    u.pp_std as pp,
    u.std_weighted_map_length as map_length,
    unixepoch(u.last_update) as last_update
  FROM osu_users u
  WHERE u.pp_std IS NOT NULL AND u.std_weighted_map_length IS NOT NULL;

INSERT INTO osu_user_mode_stats (user_id, mode, pp, last_update)
  SELECT 
    u.user_id,
    1 as mode,
    u.pp_taiko as pp,
    unixepoch(u.last_update) as last_update
  FROM osu_users u
  WHERE u.pp_taiko IS NOT NULL;

INSERT INTO osu_user_mode_stats (user_id, mode, pp, last_update)
  SELECT 
    u.user_id,
    2 as mode,
    u.pp_catch as pp,
    unixepoch(u.last_update) as last_update
  FROM osu_users u
  WHERE u.pp_catch IS NOT NULL;

INSERT INTO osu_user_mode_stats (user_id, mode, pp, last_update)
  SELECT 
    u.user_id,
    3 as mode,
    u.pp_mania as pp,
    unixepoch(u.last_update) as last_update
  FROM osu_users u
  WHERE u.pp_mania IS NOT NULL;

-- Clean up old table

ALTER TABLE osu_users DROP COLUMN last_update;
ALTER TABLE osu_users DROP COLUMN pp_std;
ALTER TABLE osu_users DROP COLUMN pp_taiko;
ALTER TABLE osu_users DROP COLUMN pp_catch;
ALTER TABLE osu_users DROP COLUMN pp_mania;
ALTER TABLE osu_users DROP COLUMN std_weighted_map_length;

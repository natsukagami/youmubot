-- Add migration script here

ALTER TABLE osu_users
  ADD COLUMN preferred_mode INT NOT NULL DEFAULT 0 CHECK (preferred_mode >= 0 AND preferred_mode < 4);

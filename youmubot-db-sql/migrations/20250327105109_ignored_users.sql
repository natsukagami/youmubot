-- Add migration script here

CREATE TABLE ignored_users (
  id BIGINT NOT NULL PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  ignored_since DATETIME NOT NULL DEFAULT (DATETIME('now'))
);

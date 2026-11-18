-- Open Budget Tracking's SQLite database, as the app makes it: schema
-- version 2. The app builds it in steps (src/db.rs), so that older databases
-- can be brought up to date. This file is the same thing in one piece, for
-- reference, and a test checks the two stay the same.
--
-- Prices are whole pence or cents, so totals never suffer from rounding. Dates
-- are YYYY-MM-DD. Category ids start at 22, as in mariadb.sql beside this file.

CREATE TABLE IF NOT EXISTS category (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    short TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(short) BETWEEN 1 AND 32)
);
CREATE TABLE IF NOT EXISTS spending (
    line   INTEGER PRIMARY KEY AUTOINCREMENT,
    name   TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    date   TEXT    NOT NULL CHECK (date(date) IS date),
    price  INTEGER NOT NULL CHECK (abs(price) <= 99999999),
    bucket INTEGER NOT NULL REFERENCES category (id)
);
CREATE INDEX IF NOT EXISTS spending_date ON spending (date);
CREATE TABLE IF NOT EXISTS setting (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO sqlite_sequence (name, seq) VALUES ('category', 21);
INSERT INTO category (short) VALUES
    ('Entertainment'),
    ('Food and Drink'),
    ('Utilities and Bills'),
    ('Web Services'),
    ('Transport'),
    ('Accommodation'),
    ('Healthcare'),
    ('Social Events');
PRAGMA user_version = 2;

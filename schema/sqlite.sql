-- SQLite equivalent of mariadb.sql. Category IDs start at 22 to match the
-- MariaDB schema (AUTO_INCREMENT = 22), so data can move between backends.

CREATE TABLE IF NOT EXISTS category (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    short VARCHAR(32) NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE IF NOT EXISTS spending (
    line   INTEGER PRIMARY KEY AUTOINCREMENT,
    name   VARCHAR(160) NOT NULL,
    date   DATE NOT NULL,
    price  DECIMAL(8,2) NOT NULL,
    bucket INT NOT NULL,
    FOREIGN KEY (bucket) REFERENCES category (id)
);

INSERT INTO category (id, short) VALUES
(22, 'Entertainment'),
(23, 'Food and Drink'),
(24, 'Utilities and Bills'),
(25, 'Web Services'),
(26, 'Transport'),
(27, 'Accommodation'),
(28, 'Healthcare'),
(29, 'Social Events');

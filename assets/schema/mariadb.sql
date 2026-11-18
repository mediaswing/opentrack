-- Open Budget Tracking's tables for MariaDB, kept as a reference in case the
-- app is ever pointed at MariaDB. The app itself uses SQLite (see sqlite.sql
-- beside this file, and src/db.rs). Prices are DECIMAL(8,2) here, and whole
-- pence or cents in SQLite.

CREATE TABLE IF NOT EXISTS `category` (
    `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `short` VARCHAR(32) NOT NULL UNIQUE
) ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS `spending` (
    `line` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `name` VARCHAR(160) NOT NULL,
    `date` DATE NOT NULL,
    `price` DECIMAL(8,2) NOT NULL,
    `bucket` INT NOT NULL,
    FOREIGN KEY (`bucket`) REFERENCES `category` (`id`),
    INDEX `spending_date` (`date`)
) ENGINE = InnoDB;

-- The choices made on the Settings tab, such as the appearance and locale.
CREATE TABLE IF NOT EXISTS `setting` (
    `name` VARCHAR(64) NOT NULL PRIMARY KEY,
    `value` TEXT NOT NULL
) ENGINE = InnoDB;

ALTER TABLE `category` AUTO_INCREMENT = 22;

INSERT INTO `category` VALUES
(NULL, 'Entertainment'),
(NULL, 'Food and Drink'),
(NULL, 'Utilities and Bills'),
(NULL, 'Web Services'),
(NULL, 'Transport'),
(NULL, 'Accommodation'),
(NULL, 'Healthcare'),
(NULL, 'Social Events');

CREATE TABLE IF NOT EXISTS `category` (
    `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `short` VARCHAR(32) NOT NULL UNIQUE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- Teams let people share transactions on a shared database. The app only
-- reads them; the database's administrator adds teams and their members.
CREATE TABLE IF NOT EXISTS `team` (
    `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `name` VARCHAR(64) NOT NULL UNIQUE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- `member` is a user's identity as the app shows it under Settings: a SID
-- (S-1-5-21-...) on Windows, login@computer on macOS and Linux, or one they chose. Identities
-- are compared exactly, so `Sam` and `sam` are different people.
CREATE TABLE IF NOT EXISTS `team_member` (
    `team` INT NOT NULL,
    `member` VARCHAR(190) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
    PRIMARY KEY (`team`, `member`),
    KEY (`member`),
    FOREIGN KEY (`team`) REFERENCES `team` (`id`) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- `owner` is who added the row (NULL for rows from before owners were
-- recorded, which everyone can see). `team`, if set, shares it with that team's members.
-- Databases made by older versions get these columns when the app connects.
CREATE TABLE IF NOT EXISTS `spending` (
    `line` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `name` VARCHAR(160) NOT NULL,
    `date` DATE NOT NULL,
    `price` DECIMAL(8,2) NOT NULL,
    `bucket` INT NOT NULL,
    `owner` VARCHAR(190) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NULL,
    `team` INT NULL,
    KEY (`owner`),
    FOREIGN KEY (`bucket`) REFERENCES `category` (`id`),
    FOREIGN KEY (`team`) REFERENCES `team` (`id`) ON DELETE SET NULL
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

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

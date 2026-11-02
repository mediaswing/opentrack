CREATE TABLE IF NOT EXISTS `category` (
    `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `short` VARCHAR(32) NOT NULL UNIQUE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

CREATE TABLE IF NOT EXISTS `spending` (
    `line` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `name` VARCHAR(160) NOT NULL,
    `date` DATE NOT NULL,
    `price` DECIMAL(8,2) NOT NULL,
    `bucket` INT NOT NULL,
    FOREIGN KEY (`bucket`) REFERENCES `category` (`id`)
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
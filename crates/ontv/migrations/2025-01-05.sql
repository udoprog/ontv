CREATE TABLE `collections` (
    `id` INTEGER PRIMARY KEY AUTOINCREMENT,
    `name` TEXT NOT NULL,
    `description` TEXT NOT NULL,
    `created_at` INTEGER NOT NULL,
    `updated_at` INTEGER NOT NULL
);

CREATE TABLE `absolute_episodes` (
    `id` BLOB PRIMARY KEY,
    `series_id` BLOB NOT NULL,
    `collection_id` INTEGER NOT NULL,
    `episode_id` BLOB NOT NULL,
    `absolute_number` INTEGER NOT NULL,
    FOREIGN KEY (`collection_id`) REFERENCES `collections` (`id`) ON DELETE CASCADE
);

-- Create an index which makes it faster to look up episodes by series and absolute number.
CREATE INDEX `idx_episodes_series_absolute` ON `absolute_episodes` (`series_id`, `collection_id`, `absolute_number`);

CREATE TABLE `season_episodes` (
    `id` BLOB PRIMARY KEY,
    `series_id` BLOB NOT NULL,
    `collection_id` INTEGER NOT NULL,
    `episode_id` BLOB NOT NULL,
    `season_number` INTEGER NOT NULL,
    `episode_number` INTEGER NOT NULL,
    FOREIGN KEY (`collection_id`) REFERENCES `collections` (`id`) ON DELETE CASCADE
);

-- Create an index which makes it faster to look up episodes by series, season, and episode number.
CREATE INDEX `idx_episodes_series_season_episode` ON `season_episodes` (`series_id`, `collection_id`, `season_number`, `episode_number`);

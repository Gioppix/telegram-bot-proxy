-- Subscriptions table
CREATE TABLE subscriptions
(
    id           integer PRIMARY KEY NOT NULL,
    telegram_id  integer             NOT NULL,
    channel_name text                NOT NULL CHECK (channel_name NOT LIKE '% %' AND LENGTH(channel_name) > 0),
    created_at   integer             NOT NULL DEFAULT (unixepoch())
) STRICT;

-- Ensure one user can't subscribe to the same channel twice
CREATE UNIQUE INDEX idx_telegram_channel ON subscriptions (telegram_id, channel_name);

-- Index for fast lookups by channel
CREATE INDEX idx_channel ON subscriptions (channel_name);

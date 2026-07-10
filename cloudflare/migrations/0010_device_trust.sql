CREATE TABLE IF NOT EXISTS user_device_keys (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  signing_public_key TEXT NOT NULL,
  wrapping_public_key TEXT,
  key_protocol_version INTEGER NOT NULL CHECK (key_protocol_version IN (1, 2)),
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, device_id),
  FOREIGN KEY (user_id, device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  CHECK (
    (key_protocol_version = 1 AND wrapping_public_key IS NULL)
    OR
    (
      key_protocol_version = 2
      AND length(signing_public_key) = 64
      AND signing_public_key NOT GLOB '*[^0-9a-f]*'
      AND wrapping_public_key IS NOT NULL
      AND length(wrapping_public_key) = 64
      AND wrapping_public_key NOT GLOB '*[^0-9a-f]*'
    )
  )
);

INSERT OR IGNORE INTO user_device_keys (
  user_id,
  device_id,
  signing_public_key,
  wrapping_public_key,
  key_protocol_version,
  created_at
)
SELECT user_id, device_id, public_key, NULL, 1, created_at
FROM user_devices;

CREATE TABLE IF NOT EXISTS device_rebind_challenges (
  challenge_id TEXT NOT NULL PRIMARY KEY,
  user_id TEXT NOT NULL,
  session_id TEXT NOT NULL UNIQUE,
  device_id TEXT NOT NULL,
  challenge TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  consumed_at INTEGER,
  consumption_nonce TEXT,
  FOREIGN KEY (session_id) REFERENCES better_auth_session (id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, device_id)
    REFERENCES user_device_keys (user_id, device_id) ON DELETE CASCADE,
  CHECK (expires_at > created_at),
  CHECK (
    (consumed_at IS NULL AND consumption_nonce IS NULL)
    OR (consumed_at IS NOT NULL AND consumption_nonce IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_device_rebind_challenges_expiry
  ON device_rebind_challenges (expires_at, consumed_at);

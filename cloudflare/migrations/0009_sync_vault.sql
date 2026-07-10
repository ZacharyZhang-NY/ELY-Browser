CREATE TABLE IF NOT EXISTS sync_vault_accounts (
  user_id TEXT NOT NULL PRIMARY KEY,
  current_key_id TEXT NOT NULL
    CHECK (length(current_key_id) = 64 AND current_key_id NOT GLOB '*[^0-9a-f]*'),
  current_generation INTEGER NOT NULL CHECK (current_generation >= 1),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
  FOREIGN KEY (user_id) REFERENCES better_auth_user (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sync_vault_envelopes (
  user_id TEXT NOT NULL,
  recipient_device_id TEXT NOT NULL,
  approver_device_id TEXT NOT NULL,
  key_id TEXT NOT NULL
    CHECK (length(key_id) = 64 AND key_id NOT GLOB '*[^0-9a-f]*'),
  generation INTEGER NOT NULL CHECK (generation >= 1),
  envelope_version INTEGER NOT NULL CHECK (envelope_version = 1),
  suite TEXT NOT NULL
    CHECK (suite = 'HPKE-BASE-X25519-HKDF-SHA256-CHACHA20POLY1305'),
  encapped_key TEXT NOT NULL
    CHECK (
      length(encapped_key) = 43
      AND encapped_key NOT GLOB '*[^A-Za-z0-9_-]*'
      AND substr(encapped_key, 43, 1) GLOB '[AEIMQUYcgkosw048]'
    ),
  ciphertext TEXT NOT NULL
    CHECK (length(ciphertext) = 64 AND ciphertext NOT GLOB '*[^A-Za-z0-9_-]*'),
  idempotency_key TEXT NOT NULL
    CHECK (
      length(idempotency_key) BETWEEN 16 AND 128
      AND idempotency_key NOT GLOB '*[^A-Za-z0-9._:-]*'
    ),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  PRIMARY KEY (user_id, recipient_device_id, key_id, generation),
  UNIQUE (user_id, idempotency_key),
  FOREIGN KEY (user_id) REFERENCES sync_vault_accounts (user_id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, recipient_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, approver_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sync_vault_envelopes_current_device
  ON sync_vault_envelopes (user_id, recipient_device_id, generation, key_id);

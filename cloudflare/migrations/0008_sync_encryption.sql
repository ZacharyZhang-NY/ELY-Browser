CREATE TABLE IF NOT EXISTS sync_snapshot_encryption (
  user_id TEXT NOT NULL,
  snapshot_id TEXT NOT NULL,
  encryption_version INTEGER NOT NULL CHECK (encryption_version = 1),
  vault_generation INTEGER NOT NULL CHECK (vault_generation >= 1),
  key_id TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  PRIMARY KEY (user_id, snapshot_id),
  FOREIGN KEY (user_id, snapshot_id) REFERENCES sync_snapshots (user_id, snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_snapshots_encrypted_latest
  ON sync_snapshot_encryption (user_id, encryption_version, snapshot_id);

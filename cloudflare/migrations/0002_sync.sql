CREATE TABLE IF NOT EXISTS sync_objects (
  user_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  object_type TEXT NOT NULL,
  payload_inline BLOB,
  payload_r2_key TEXT,
  payload_hash TEXT NOT NULL,
  schema_rev INTEGER NOT NULL,
  logical_clock INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER,
  PRIMARY KEY (user_id, object_id),
  CHECK (payload_inline IS NULL OR payload_r2_key IS NULL),
  CHECK (deleted_at IS NOT NULL OR payload_inline IS NOT NULL OR payload_r2_key IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_sync_objects_delta
  ON sync_objects (user_id, object_type, logical_clock, updated_at);

CREATE TABLE IF NOT EXISTS sync_change_log (
  change_id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  object_type TEXT NOT NULL,
  operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
  payload_hash TEXT NOT NULL,
  logical_clock INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (user_id, object_id, logical_clock, device_id, operation)
);

CREATE INDEX IF NOT EXISTS idx_sync_change_log_pull
  ON sync_change_log (user_id, change_id);

CREATE TABLE IF NOT EXISTS sync_snapshots (
  user_id TEXT NOT NULL,
  snapshot_id TEXT NOT NULL,
  r2_key TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  schema_rev INTEGER NOT NULL,
  logical_clock INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, snapshot_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_snapshots_latest
  ON sync_snapshots (user_id, created_at);

CREATE TABLE IF NOT EXISTS sync_tombstones (
  user_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  object_type TEXT NOT NULL,
  logical_clock INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  deleted_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, object_id)
);

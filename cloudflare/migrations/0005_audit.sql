CREATE TABLE IF NOT EXISTS audit_events (
  event_id TEXT PRIMARY KEY,
  user_id TEXT,
  actor_device_id TEXT,
  event_type TEXT NOT NULL,
  subject_type TEXT NOT NULL,
  subject_id TEXT,
  outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
  metadata_hash TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_events_user_time
  ON audit_events (user_id, created_at);

CREATE INDEX IF NOT EXISTS idx_audit_events_type_time
  ON audit_events (event_type, created_at);

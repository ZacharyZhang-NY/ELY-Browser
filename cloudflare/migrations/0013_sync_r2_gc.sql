CREATE TABLE sync_r2_gc_candidates (
  r2_key TEXT NOT NULL PRIMARY KEY CHECK (length(r2_key) BETWEEN 1 AND 1024),
  user_id TEXT,
  owner_hash TEXT NOT NULL
    CHECK (length(owner_hash) = 64 AND owner_hash NOT GLOB '*[^0-9a-f]*'),
  object_kind TEXT NOT NULL CHECK (object_kind IN ('payload', 'snapshot')),
  state TEXT NOT NULL CHECK (state IN ('pending', 'referenced', 'ready', 'deleting', 'deleted')),
  write_token TEXT
    CHECK (
      write_token IS NULL
      OR (length(write_token) = 64 AND write_token NOT GLOB '*[^0-9a-f]*')
    ),
  lease_expires_at INTEGER NOT NULL CHECK (lease_expires_at >= 0),
  gc_token TEXT
    CHECK (
      gc_token IS NULL
      OR (length(gc_token) = 64 AND gc_token NOT GLOB '*[^0-9a-f]*')
    ),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  updated_at INTEGER NOT NULL CHECK (updated_at >= created_at),
  referenced_at INTEGER,
  ready_at INTEGER,
  delete_started_at INTEGER,
  deleted_at INTEGER,
  CHECK (state <> 'pending' OR (user_id IS NOT NULL AND write_token IS NOT NULL)),
  CHECK (state <> 'referenced' OR referenced_at IS NOT NULL),
  CHECK (state NOT IN ('ready', 'deleting', 'deleted') OR ready_at IS NOT NULL),
  CHECK (state NOT IN ('deleting', 'deleted') OR (gc_token IS NOT NULL AND delete_started_at IS NOT NULL)),
  CHECK (state <> 'deleted' OR deleted_at IS NOT NULL)
);

CREATE INDEX idx_sync_r2_gc_ready
  ON sync_r2_gc_candidates (state, lease_expires_at, delete_started_at, updated_at);

CREATE INDEX idx_sync_r2_gc_user
  ON sync_r2_gc_candidates (user_id, state, updated_at);

CREATE INDEX idx_sync_r2_gc_owner
  ON sync_r2_gc_candidates (owner_hash, state, updated_at);

CREATE TABLE sync_r2_inventory_cursors (
  prefix TEXT NOT NULL PRIMARY KEY CHECK (prefix IN ('sync-payloads/', 'sync-snapshots/')),
  cursor TEXT,
  updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
  next_scan_at INTEGER NOT NULL CHECK (next_scan_at >= updated_at)
);

INSERT INTO sync_r2_inventory_cursors (prefix, cursor, updated_at, next_scan_at)
VALUES
  ('sync-payloads/', NULL, 0, 0),
  ('sync-snapshots/', NULL, 0, 0);

INSERT OR IGNORE INTO sync_r2_gc_candidates (
  r2_key, user_id, owner_hash, object_kind, state, write_token,
  lease_expires_at, gc_token, created_at, updated_at, referenced_at,
  ready_at, delete_started_at, deleted_at
)
SELECT
  object.payload_r2_key,
  object.user_id,
  substr(
    object.payload_r2_key,
    instr(object.payload_r2_key, '/')
      + instr(substr(object.payload_r2_key, instr(object.payload_r2_key, '/') + 1), '/')
      + 1,
    64
  ),
  'payload',
  'referenced',
  NULL,
  0,
  NULL,
  object.created_at,
  object.updated_at,
  object.updated_at,
  NULL,
  NULL,
  NULL
FROM sync_objects AS object
WHERE object.payload_r2_key IS NOT NULL;

INSERT OR IGNORE INTO sync_r2_gc_candidates (
  r2_key, user_id, owner_hash, object_kind, state, write_token,
  lease_expires_at, gc_token, created_at, updated_at, referenced_at,
  ready_at, delete_started_at, deleted_at
)
SELECT
  snapshot.r2_key,
  snapshot.user_id,
  substr(
    snapshot.r2_key,
    instr(snapshot.r2_key, '/')
      + instr(substr(snapshot.r2_key, instr(snapshot.r2_key, '/') + 1), '/')
      + 1,
    64
  ),
  'snapshot',
  'referenced',
  NULL,
  0,
  NULL,
  snapshot.created_at,
  snapshot.created_at,
  snapshot.created_at,
  NULL,
  NULL,
  NULL
FROM sync_snapshots AS snapshot;

CREATE TRIGGER sync_r2_gc_state_transition_guard
BEFORE UPDATE OF state ON sync_r2_gc_candidates
FOR EACH ROW
WHEN NOT (
  OLD.state = NEW.state
  OR (OLD.state = 'pending' AND NEW.state IN ('referenced', 'ready', 'deleting'))
  OR (OLD.state = 'referenced' AND NEW.state = 'ready')
  OR (OLD.state = 'ready' AND NEW.state = 'deleting')
  OR (OLD.state = 'deleting' AND NEW.state = 'deleted')
  OR (OLD.state = 'deleted' AND NEW.state = 'ready')
)
BEGIN
  SELECT RAISE(ABORT, 'sync_r2_gc_state_transition_invalid');
END;

CREATE TRIGGER sync_r2_snapshot_insert_fence
BEFORE INSERT ON sync_snapshots
FOR EACH ROW
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM sync_r2_gc_candidates AS candidate
    WHERE candidate.r2_key = NEW.r2_key
      AND candidate.user_id = NEW.user_id
      AND candidate.object_kind = 'snapshot'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_snapshot_update_fence
BEFORE UPDATE OF r2_key, payload_hash, head_revision ON sync_snapshots
FOR EACH ROW
WHEN OLD.r2_key <> NEW.r2_key
  OR OLD.payload_hash <> NEW.payload_hash
  OR OLD.head_revision <> NEW.head_revision
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM sync_r2_gc_candidates AS candidate
    WHERE candidate.r2_key = NEW.r2_key
      AND candidate.user_id = NEW.user_id
      AND candidate.object_kind = 'snapshot'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_snapshot_head_insert_fence
BEFORE INSERT ON sync_snapshot_heads
FOR EACH ROW
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM sync_snapshots AS snapshot
    INNER JOIN sync_r2_gc_candidates AS candidate
      ON candidate.r2_key = snapshot.r2_key
      AND candidate.user_id = snapshot.user_id
      AND candidate.object_kind = 'snapshot'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
    WHERE snapshot.user_id = NEW.user_id
      AND snapshot.snapshot_id = NEW.snapshot_id
      AND snapshot.head_revision = NEW.head_revision
      AND snapshot.payload_hash = NEW.payload_hash
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_snapshot_head_update_fence
BEFORE UPDATE ON sync_snapshot_heads
FOR EACH ROW
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM sync_snapshots AS snapshot
    INNER JOIN sync_r2_gc_candidates AS candidate
      ON candidate.r2_key = snapshot.r2_key
      AND candidate.user_id = snapshot.user_id
      AND candidate.object_kind = 'snapshot'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
    WHERE snapshot.user_id = NEW.user_id
      AND snapshot.snapshot_id = NEW.snapshot_id
      AND snapshot.head_revision = NEW.head_revision
      AND snapshot.payload_hash = NEW.payload_hash
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_payload_insert_fence
BEFORE INSERT ON sync_objects
FOR EACH ROW
WHEN NEW.payload_r2_key IS NOT NULL
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM sync_r2_gc_candidates AS candidate
    WHERE candidate.r2_key = NEW.payload_r2_key
      AND candidate.user_id = NEW.user_id
      AND candidate.object_kind = 'payload'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_payload_update_fence
BEFORE UPDATE OF payload_r2_key ON sync_objects
FOR EACH ROW
WHEN NEW.payload_r2_key IS NOT NULL
  AND OLD.payload_r2_key IS NOT NEW.payload_r2_key
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM sync_r2_gc_candidates AS candidate
    WHERE candidate.r2_key = NEW.payload_r2_key
      AND candidate.user_id = NEW.user_id
      AND candidate.object_kind = 'payload'
      AND candidate.state = 'pending'
      AND candidate.write_token IS NOT NULL
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_write_fenced') END;
END;

CREATE TRIGGER sync_r2_snapshot_mark_referenced_guard
BEFORE UPDATE OF state ON sync_r2_gc_candidates
FOR EACH ROW
WHEN OLD.object_kind = 'snapshot'
  AND OLD.state = 'pending'
  AND NEW.state = 'referenced'
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM sync_snapshot_heads AS head
    INNER JOIN sync_snapshots AS snapshot
      ON snapshot.user_id = head.user_id
      AND snapshot.snapshot_id = head.snapshot_id
      AND snapshot.head_revision = head.head_revision
      AND snapshot.payload_hash = head.payload_hash
    WHERE snapshot.r2_key = OLD.r2_key
      AND snapshot.user_id = OLD.user_id
  ) THEN 1 ELSE RAISE(ABORT, 'sync_r2_reference_commit_invalid') END;
END;

CREATE TRIGGER sync_r2_snapshot_displaced
AFTER UPDATE OF r2_key ON sync_snapshots
FOR EACH ROW
WHEN OLD.r2_key <> NEW.r2_key
BEGIN
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    updated_at = MAX(updated_at, unixepoch()),
    ready_at = COALESCE(ready_at, unixepoch())
  WHERE r2_key = OLD.r2_key
    AND state = 'referenced'
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = OLD.r2_key)
    AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = OLD.r2_key);
END;

CREATE TRIGGER sync_r2_snapshot_deleted
AFTER DELETE ON sync_snapshots
FOR EACH ROW
BEGIN
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    updated_at = MAX(updated_at, unixepoch()),
    ready_at = COALESCE(ready_at, unixepoch())
  WHERE r2_key = OLD.r2_key
    AND state = 'referenced'
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = OLD.r2_key)
    AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = OLD.r2_key);
END;

CREATE TRIGGER sync_r2_payload_displaced
AFTER UPDATE OF payload_r2_key ON sync_objects
FOR EACH ROW
WHEN OLD.payload_r2_key IS NOT NULL
  AND OLD.payload_r2_key IS NOT NEW.payload_r2_key
BEGIN
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    updated_at = MAX(updated_at, unixepoch()),
    ready_at = COALESCE(ready_at, unixepoch())
  WHERE r2_key = OLD.payload_r2_key
    AND state = 'referenced'
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = OLD.payload_r2_key)
    AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = OLD.payload_r2_key);
END;

CREATE TRIGGER sync_r2_payload_deleted
AFTER DELETE ON sync_objects
FOR EACH ROW
WHEN OLD.payload_r2_key IS NOT NULL
BEGIN
  UPDATE sync_r2_gc_candidates
  SET
    state = 'ready',
    updated_at = MAX(updated_at, unixepoch()),
    ready_at = COALESCE(ready_at, unixepoch())
  WHERE r2_key = OLD.payload_r2_key
    AND state = 'referenced'
    AND NOT EXISTS (SELECT 1 FROM sync_snapshots WHERE r2_key = OLD.payload_r2_key)
    AND NOT EXISTS (SELECT 1 FROM sync_objects WHERE payload_r2_key = OLD.payload_r2_key);
END;

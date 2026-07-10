ALTER TABLE sync_snapshots
  ADD COLUMN head_revision INTEGER NOT NULL DEFAULT 0 CHECK (head_revision >= 0);

ALTER TABLE sync_snapshots
  ADD COLUMN base_head_revision INTEGER CHECK (base_head_revision IS NULL OR base_head_revision >= 1);

ALTER TABLE sync_snapshots
  ADD COLUMN base_snapshot_id TEXT;

ALTER TABLE sync_snapshots
  ADD COLUMN base_payload_hash TEXT
    CHECK (
      base_payload_hash IS NULL
      OR (
        length(base_payload_hash) = 64
        AND base_payload_hash NOT GLOB '*[^0-9a-f]*'
      )
    );

ALTER TABLE sync_snapshot_encryption RENAME TO sync_snapshot_encryption_v1;

DROP INDEX IF EXISTS idx_sync_snapshots_encrypted_latest;

CREATE TABLE sync_snapshot_encryption (
  user_id TEXT NOT NULL,
  snapshot_id TEXT NOT NULL,
  encryption_version INTEGER NOT NULL CHECK (encryption_version IN (1, 2)),
  vault_generation INTEGER NOT NULL CHECK (vault_generation >= 1),
  key_id TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  PRIMARY KEY (user_id, snapshot_id),
  FOREIGN KEY (user_id, snapshot_id) REFERENCES sync_snapshots (user_id, snapshot_id)
);

INSERT INTO sync_snapshot_encryption (
  user_id,
  snapshot_id,
  encryption_version,
  vault_generation,
  key_id,
  content_hash
)
SELECT
  user_id,
  snapshot_id,
  encryption_version,
  vault_generation,
  key_id,
  content_hash
FROM sync_snapshot_encryption_v1;

DROP TABLE sync_snapshot_encryption_v1;

CREATE INDEX idx_sync_snapshots_encrypted_latest
  ON sync_snapshot_encryption (user_id, encryption_version, snapshot_id);

UPDATE sync_snapshots AS candidate
SET head_revision = 1
WHERE EXISTS (
    SELECT 1
    FROM sync_snapshot_encryption AS encryption
    WHERE encryption.user_id = candidate.user_id
      AND encryption.snapshot_id = candidate.snapshot_id
      AND encryption.encryption_version = 1
  )
  AND NOT EXISTS (
    SELECT 1
    FROM sync_snapshots AS newer
    INNER JOIN sync_snapshot_encryption AS newer_encryption
      ON newer_encryption.user_id = newer.user_id
      AND newer_encryption.snapshot_id = newer.snapshot_id
      AND newer_encryption.encryption_version = 1
    WHERE newer.user_id = candidate.user_id
      AND (
        newer.created_at > candidate.created_at
        OR (
          newer.created_at = candidate.created_at
          AND newer.snapshot_id < candidate.snapshot_id
        )
      )
  );

CREATE UNIQUE INDEX idx_sync_snapshot_committed_revision
  ON sync_snapshots (user_id, head_revision)
  WHERE head_revision > 0;

CREATE TABLE sync_snapshot_heads (
  user_id TEXT NOT NULL PRIMARY KEY,
  head_revision INTEGER NOT NULL CHECK (head_revision >= 1),
  snapshot_id TEXT NOT NULL,
  payload_hash TEXT NOT NULL
    CHECK (length(payload_hash) = 64 AND payload_hash NOT GLOB '*[^0-9a-f]*'),
  updated_at INTEGER NOT NULL CHECK (updated_at >= 0),
  FOREIGN KEY (user_id, snapshot_id)
    REFERENCES sync_snapshots (user_id, snapshot_id) ON DELETE RESTRICT,
  FOREIGN KEY (user_id, snapshot_id)
    REFERENCES sync_snapshot_encryption (user_id, snapshot_id) ON DELETE RESTRICT
);

INSERT INTO sync_snapshot_heads (
  user_id,
  head_revision,
  snapshot_id,
  payload_hash,
  updated_at
)
SELECT user_id, 1, snapshot_id, payload_hash, created_at
FROM sync_snapshots
WHERE head_revision = 1;

CREATE TRIGGER sync_snapshot_candidate_insert_guard
BEFORE INSERT ON sync_snapshots
FOR EACH ROW
WHEN NEW.head_revision > 0
BEGIN
  SELECT CASE WHEN (
    (
      NEW.head_revision = 1
      AND NEW.base_head_revision IS NULL
      AND NEW.base_snapshot_id IS NULL
      AND NEW.base_payload_hash IS NULL
      AND NOT EXISTS (
        SELECT 1 FROM sync_snapshot_heads WHERE user_id = NEW.user_id
      )
    )
    OR
    (
      NEW.head_revision > 1
      AND NEW.base_head_revision IS NOT NULL
      AND NEW.base_snapshot_id IS NOT NULL
      AND NEW.base_payload_hash IS NOT NULL
      AND EXISTS (
        SELECT 1
        FROM sync_snapshot_heads AS head
        INNER JOIN sync_snapshots AS base
          ON base.user_id = head.user_id
          AND base.snapshot_id = head.snapshot_id
          AND base.payload_hash = head.payload_hash
        WHERE head.user_id = NEW.user_id
          AND head.head_revision = NEW.base_head_revision
          AND head.snapshot_id = NEW.base_snapshot_id
          AND head.payload_hash = NEW.base_payload_hash
          AND NEW.head_revision = head.head_revision + 1
          AND NEW.logical_clock > base.logical_clock
      )
    )
  ) THEN 1 ELSE RAISE(ABORT, 'sync_snapshot_head_cas_failed') END;
END;

CREATE TRIGGER sync_snapshot_candidate_update_guard
BEFORE UPDATE ON sync_snapshots
FOR EACH ROW
WHEN OLD.head_revision > 0 OR NEW.head_revision > 0
BEGIN
  SELECT CASE WHEN (
    (
      NEW.head_revision = 1
      AND NEW.base_head_revision IS NULL
      AND NEW.base_snapshot_id IS NULL
      AND NEW.base_payload_hash IS NULL
      AND NOT EXISTS (
        SELECT 1 FROM sync_snapshot_heads WHERE user_id = NEW.user_id
      )
    )
    OR
    (
      NEW.head_revision > 1
      AND NEW.base_head_revision IS NOT NULL
      AND NEW.base_snapshot_id IS NOT NULL
      AND NEW.base_payload_hash IS NOT NULL
      AND EXISTS (
        SELECT 1
        FROM sync_snapshot_heads AS head
        INNER JOIN sync_snapshots AS base
          ON base.user_id = head.user_id
          AND base.snapshot_id = head.snapshot_id
          AND base.payload_hash = head.payload_hash
        WHERE head.user_id = NEW.user_id
          AND head.head_revision = NEW.base_head_revision
          AND head.snapshot_id = NEW.base_snapshot_id
          AND head.payload_hash = NEW.base_payload_hash
          AND NEW.head_revision = head.head_revision + 1
          AND NEW.logical_clock > base.logical_clock
      )
    )
  ) THEN 1 ELSE RAISE(ABORT, 'sync_snapshot_head_cas_failed') END;
END;

CREATE TRIGGER sync_snapshot_head_insert_guard
BEFORE INSERT ON sync_snapshot_heads
FOR EACH ROW
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM sync_snapshots AS snapshot
    INNER JOIN sync_snapshot_encryption AS encryption
      ON encryption.user_id = snapshot.user_id
      AND encryption.snapshot_id = snapshot.snapshot_id
    INNER JOIN sync_vault_accounts AS account
      ON account.user_id = snapshot.user_id
      AND account.current_key_id = encryption.key_id
      AND account.current_generation = encryption.vault_generation
    INNER JOIN user_devices AS device
      ON device.user_id = snapshot.user_id
      AND device.device_id = snapshot.device_id
      AND device.approval_status = 'approved'
      AND device.revoked_at IS NULL
    INNER JOIN user_device_keys AS keys
      ON keys.user_id = device.user_id
      AND keys.device_id = device.device_id
      AND keys.key_protocol_version = 2
      AND keys.wrapping_public_key IS NOT NULL
    WHERE snapshot.user_id = NEW.user_id
      AND snapshot.snapshot_id = NEW.snapshot_id
      AND snapshot.payload_hash = NEW.payload_hash
      AND snapshot.head_revision = NEW.head_revision
      AND snapshot.head_revision = 1
      AND snapshot.base_head_revision IS NULL
      AND snapshot.base_snapshot_id IS NULL
      AND snapshot.base_payload_hash IS NULL
      AND encryption.encryption_version = 2
      AND NOT EXISTS (
        SELECT 1
        FROM sync_vault_rotation_r2_objects AS staged
        INNER JOIN sync_vault_rotations AS rotation
          ON rotation.user_id = staged.user_id
          AND rotation.idempotency_key = staged.rotation_idempotency_key
        WHERE staged.user_id = snapshot.user_id
          AND staged.r2_key = snapshot.r2_key
          AND rotation.cleanup_started_at IS NOT NULL
      )
  ) THEN 1 ELSE RAISE(ABORT, 'sync_snapshot_head_cas_failed') END;
END;

CREATE TRIGGER sync_snapshot_head_update_guard
BEFORE UPDATE ON sync_snapshot_heads
FOR EACH ROW
BEGIN
  SELECT CASE WHEN (
    NEW.user_id = OLD.user_id
    AND NEW.head_revision = OLD.head_revision + 1
    AND EXISTS (
      SELECT 1
      FROM sync_snapshots AS snapshot
      INNER JOIN sync_snapshot_encryption AS encryption
        ON encryption.user_id = snapshot.user_id
        AND encryption.snapshot_id = snapshot.snapshot_id
      INNER JOIN sync_vault_accounts AS account
        ON account.user_id = snapshot.user_id
        AND account.current_key_id = encryption.key_id
        AND account.current_generation = encryption.vault_generation
      INNER JOIN user_devices AS device
        ON device.user_id = snapshot.user_id
        AND device.device_id = snapshot.device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id
        AND keys.device_id = device.device_id
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
      WHERE snapshot.user_id = NEW.user_id
        AND snapshot.snapshot_id = NEW.snapshot_id
        AND snapshot.payload_hash = NEW.payload_hash
        AND snapshot.head_revision = NEW.head_revision
        AND snapshot.base_head_revision = OLD.head_revision
        AND snapshot.base_snapshot_id = OLD.snapshot_id
        AND snapshot.base_payload_hash = OLD.payload_hash
        AND encryption.encryption_version = 2
        AND NOT EXISTS (
          SELECT 1
          FROM sync_vault_rotation_r2_objects AS staged
          INNER JOIN sync_vault_rotations AS rotation
            ON rotation.user_id = staged.user_id
            AND rotation.idempotency_key = staged.rotation_idempotency_key
          WHERE staged.user_id = snapshot.user_id
            AND staged.r2_key = snapshot.r2_key
            AND rotation.cleanup_started_at IS NOT NULL
        )
    )
  ) THEN 1 ELSE RAISE(ABORT, 'sync_snapshot_head_cas_failed') END;
END;

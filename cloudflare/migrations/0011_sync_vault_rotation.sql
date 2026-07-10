CREATE TABLE IF NOT EXISTS sync_vault_rotations (
  user_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL
    CHECK (
      length(idempotency_key) BETWEEN 16 AND 128
      AND idempotency_key NOT GLOB '*[^A-Za-z0-9._:-]*'
    ),
  audit_event_id TEXT NOT NULL UNIQUE,
  target_device_id TEXT NOT NULL,
  approver_device_id TEXT NOT NULL,
  previous_key_id TEXT NOT NULL
    CHECK (length(previous_key_id) = 64 AND previous_key_id NOT GLOB '*[^0-9a-f]*'),
  previous_generation INTEGER NOT NULL CHECK (previous_generation >= 1),
  new_key_id TEXT NOT NULL
    CHECK (length(new_key_id) = 64 AND new_key_id NOT GLOB '*[^0-9a-f]*'),
  new_generation INTEGER NOT NULL CHECK (new_generation >= 2),
  request_hash TEXT NOT NULL
    CHECK (length(request_hash) = 64 AND request_hash NOT GLOB '*[^0-9a-f]*'),
  envelope_count INTEGER NOT NULL CHECK (envelope_count BETWEEN 1 AND 128),
  r2_object_count INTEGER NOT NULL CHECK (r2_object_count >= 0),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= created_at),
  cleanup_snapshot_id TEXT
    CHECK (
      cleanup_snapshot_id IS NULL
      OR (
        length(cleanup_snapshot_id) BETWEEN 1 AND 128
        AND substr(cleanup_snapshot_id, 1, 1) GLOB '[a-z0-9]'
        AND cleanup_snapshot_id NOT GLOB '*[^a-z0-9._-]*'
      )
    ),
  cleanup_started_at INTEGER,
  storage_cleaned_at INTEGER
    CHECK (
      storage_cleaned_at IS NULL
      OR (
        cleanup_started_at IS NOT NULL
        AND storage_cleaned_at >= cleanup_started_at
      )
    ),
  PRIMARY KEY (user_id, idempotency_key),
  FOREIGN KEY (user_id) REFERENCES sync_vault_accounts (user_id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, target_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, approver_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  CHECK (target_device_id <> approver_device_id),
  CHECK (new_key_id <> previous_key_id),
  CHECK (new_generation = previous_generation + 1),
  CHECK (
    (cleanup_snapshot_id IS NULL AND cleanup_started_at IS NULL)
    OR (
      cleanup_snapshot_id IS NOT NULL
      AND cleanup_started_at IS NOT NULL
      AND completed_at IS NOT NULL
      AND cleanup_started_at >= completed_at
    )
  )
);

CREATE TABLE IF NOT EXISTS sync_vault_rotation_envelopes (
  user_id TEXT NOT NULL,
  rotation_idempotency_key TEXT NOT NULL,
  recipient_device_id TEXT NOT NULL,
  envelope_idempotency_key TEXT NOT NULL
    CHECK (
      length(envelope_idempotency_key) = 64
      AND envelope_idempotency_key NOT GLOB '*[^0-9a-f]*'
    ),
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
  PRIMARY KEY (user_id, rotation_idempotency_key, recipient_device_id),
  UNIQUE (user_id, envelope_idempotency_key),
  FOREIGN KEY (user_id, rotation_idempotency_key)
    REFERENCES sync_vault_rotations (user_id, idempotency_key) ON DELETE CASCADE,
  FOREIGN KEY (user_id, recipient_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sync_vault_rotation_r2_objects (
  user_id TEXT NOT NULL,
  rotation_idempotency_key TEXT NOT NULL,
  r2_key TEXT NOT NULL CHECK (length(r2_key) BETWEEN 1 AND 1024),
  PRIMARY KEY (user_id, rotation_idempotency_key, r2_key),
  FOREIGN KEY (user_id, rotation_idempotency_key)
    REFERENCES sync_vault_rotations (user_id, idempotency_key) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS pending_device_revocations (
  user_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL
    CHECK (
      length(idempotency_key) BETWEEN 16 AND 128
      AND idempotency_key NOT GLOB '*[^A-Za-z0-9._:-]*'
    ),
  audit_event_id TEXT NOT NULL UNIQUE,
  target_device_id TEXT NOT NULL,
  approver_device_id TEXT NOT NULL,
  request_hash TEXT NOT NULL
    CHECK (length(request_hash) = 64 AND request_hash NOT GLOB '*[^0-9a-f]*'),
  created_at INTEGER NOT NULL CHECK (created_at >= 0),
  completed_at INTEGER CHECK (completed_at IS NULL OR completed_at >= created_at),
  PRIMARY KEY (user_id, idempotency_key),
  FOREIGN KEY (user_id, target_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, approver_device_id)
    REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE,
  CHECK (target_device_id <> approver_device_id)
);

UPDATE user_devices
SET
  approval_status = 'revoked',
  revoked_at = COALESCE(revoked_at, unixepoch())
WHERE approval_status = 'approved'
  AND EXISTS (
    SELECT 1 FROM user_device_keys AS keys
    WHERE keys.user_id = user_devices.user_id
      AND keys.device_id = user_devices.device_id
      AND keys.key_protocol_version = 1
  );

CREATE TRIGGER IF NOT EXISTS finalize_pending_device_revocation
BEFORE UPDATE OF completed_at ON pending_device_revocations
FOR EACH ROW
WHEN OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL
BEGIN
  SELECT CASE WHEN
    NOT EXISTS (
      SELECT 1
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id AND keys.device_id = device.device_id
      WHERE device.user_id = OLD.user_id
        AND device.device_id = OLD.approver_device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
    )
    OR NOT EXISTS (
      SELECT 1 FROM user_devices
      WHERE user_id = OLD.user_id
        AND device_id = OLD.target_device_id
        AND approval_status = 'pending'
        AND revoked_at IS NULL
    )
  THEN RAISE(ABORT, 'pending_device_revocation_guard_failed') END;

  DELETE FROM better_auth_session
  WHERE id IN (
    SELECT session_id FROM better_auth_session_device_context
    WHERE user_id = OLD.user_id AND device_id = OLD.target_device_id
  );

  UPDATE user_devices
  SET approval_status = 'revoked', revoked_at = NEW.completed_at
  WHERE user_id = OLD.user_id
    AND device_id = OLD.target_device_id
    AND approval_status = 'pending'
    AND revoked_at IS NULL;

  INSERT INTO audit_events (
    event_id, user_id, actor_device_id, event_type, subject_type,
    subject_id, outcome, metadata_hash, created_at
  ) VALUES (
    OLD.audit_event_id, OLD.user_id, OLD.approver_device_id, 'device.revoke', 'device',
    OLD.target_device_id, 'success', OLD.request_hash, NEW.completed_at
  );
END;

CREATE TRIGGER IF NOT EXISTS finalize_sync_vault_rotation
BEFORE UPDATE OF completed_at ON sync_vault_rotations
FOR EACH ROW
WHEN OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL
BEGIN
  INSERT OR IGNORE INTO sync_vault_rotation_r2_objects (
    user_id, rotation_idempotency_key, r2_key
  )
  SELECT OLD.user_id, OLD.idempotency_key, current_r2.r2_key
  FROM (
    SELECT payload_r2_key AS r2_key
    FROM sync_objects
    WHERE user_id = OLD.user_id AND payload_r2_key IS NOT NULL
    UNION
    SELECT r2_key FROM sync_snapshots WHERE user_id = OLD.user_id
  ) AS current_r2;

  SELECT CASE WHEN
    NOT EXISTS (
      SELECT 1 FROM sync_vault_accounts
      WHERE user_id = OLD.user_id
        AND current_key_id = OLD.previous_key_id
        AND current_generation = OLD.previous_generation
    )
    OR NOT EXISTS (
      SELECT 1
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id AND keys.device_id = device.device_id
      WHERE device.user_id = OLD.user_id
        AND device.device_id = OLD.approver_device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
    )
    OR NOT EXISTS (
      SELECT 1 FROM user_devices
      WHERE user_id = OLD.user_id
        AND device_id = OLD.target_device_id
        AND approval_status IN ('pending', 'approved')
        AND revoked_at IS NULL
    )
    OR (
      SELECT COUNT(*) FROM sync_vault_rotation_envelopes
      WHERE user_id = OLD.user_id
        AND rotation_idempotency_key = OLD.idempotency_key
    ) <> OLD.envelope_count
    OR (
      SELECT COUNT(*)
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id AND keys.device_id = device.device_id
      WHERE device.user_id = OLD.user_id
        AND device.device_id <> OLD.target_device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
    ) <> OLD.envelope_count
    OR EXISTS (
      SELECT 1
      FROM sync_vault_rotation_envelopes AS envelope
      WHERE envelope.user_id = OLD.user_id
        AND envelope.rotation_idempotency_key = OLD.idempotency_key
        AND NOT EXISTS (
          SELECT 1
          FROM user_devices AS device
          INNER JOIN user_device_keys AS keys
            ON keys.user_id = device.user_id AND keys.device_id = device.device_id
          WHERE device.user_id = OLD.user_id
            AND device.device_id = envelope.recipient_device_id
            AND device.device_id <> OLD.target_device_id
            AND device.approval_status = 'approved'
            AND device.revoked_at IS NULL
            AND keys.key_protocol_version = 2
            AND keys.wrapping_public_key IS NOT NULL
        )
    )
    OR EXISTS (
      SELECT 1
      FROM user_devices AS device
      INNER JOIN user_device_keys AS keys
        ON keys.user_id = device.user_id AND keys.device_id = device.device_id
      WHERE device.user_id = OLD.user_id
        AND device.device_id <> OLD.target_device_id
        AND device.approval_status = 'approved'
        AND device.revoked_at IS NULL
        AND keys.key_protocol_version = 2
        AND keys.wrapping_public_key IS NOT NULL
        AND NOT EXISTS (
          SELECT 1 FROM sync_vault_rotation_envelopes AS envelope
          WHERE envelope.user_id = OLD.user_id
            AND envelope.rotation_idempotency_key = OLD.idempotency_key
            AND envelope.recipient_device_id = device.device_id
        )
    )
    OR (
      SELECT COUNT(*) FROM sync_vault_rotation_r2_objects
      WHERE user_id = OLD.user_id
        AND rotation_idempotency_key = OLD.idempotency_key
    ) <> OLD.r2_object_count
    OR (
      SELECT COUNT(*) FROM (
        SELECT payload_r2_key AS r2_key
        FROM sync_objects
        WHERE user_id = OLD.user_id AND payload_r2_key IS NOT NULL
        UNION
        SELECT r2_key FROM sync_snapshots WHERE user_id = OLD.user_id
      )
    ) <> OLD.r2_object_count
    OR EXISTS (
      SELECT 1 FROM sync_vault_rotation_r2_objects AS staged
      WHERE staged.user_id = OLD.user_id
        AND staged.rotation_idempotency_key = OLD.idempotency_key
        AND NOT EXISTS (
          SELECT 1 FROM (
            SELECT payload_r2_key AS r2_key
            FROM sync_objects
            WHERE user_id = OLD.user_id AND payload_r2_key IS NOT NULL
            UNION
            SELECT r2_key FROM sync_snapshots WHERE user_id = OLD.user_id
          ) AS current_r2
          WHERE current_r2.r2_key = staged.r2_key
        )
    )
    OR EXISTS (
      SELECT 1 FROM (
        SELECT payload_r2_key AS r2_key
        FROM sync_objects
        WHERE user_id = OLD.user_id AND payload_r2_key IS NOT NULL
        UNION
        SELECT r2_key FROM sync_snapshots WHERE user_id = OLD.user_id
      ) AS current_r2
      WHERE NOT EXISTS (
        SELECT 1 FROM sync_vault_rotation_r2_objects AS staged
        WHERE staged.user_id = OLD.user_id
          AND staged.rotation_idempotency_key = OLD.idempotency_key
          AND staged.r2_key = current_r2.r2_key
      )
    )
  THEN RAISE(ABORT, 'sync_vault_rotation_guard_failed') END;

  INSERT INTO sync_vault_envelopes (
    user_id, recipient_device_id, approver_device_id, key_id, generation,
    envelope_version, suite, encapped_key, ciphertext, idempotency_key, created_at
  )
  SELECT
    envelope.user_id,
    envelope.recipient_device_id,
    OLD.approver_device_id,
    OLD.new_key_id,
    OLD.new_generation,
    envelope.envelope_version,
    envelope.suite,
    envelope.encapped_key,
    envelope.ciphertext,
    envelope.envelope_idempotency_key,
    NEW.completed_at
  FROM sync_vault_rotation_envelopes AS envelope
  WHERE envelope.user_id = OLD.user_id
    AND envelope.rotation_idempotency_key = OLD.idempotency_key;

  UPDATE sync_vault_accounts
  SET
    current_key_id = OLD.new_key_id,
    current_generation = OLD.new_generation,
    updated_at = NEW.completed_at
  WHERE user_id = OLD.user_id
    AND current_key_id = OLD.previous_key_id
    AND current_generation = OLD.previous_generation;

  DELETE FROM better_auth_session
  WHERE id IN (
    SELECT session_id FROM better_auth_session_device_context
    WHERE user_id = OLD.user_id AND device_id = OLD.target_device_id
  );

  UPDATE user_devices
  SET approval_status = 'revoked', revoked_at = NEW.completed_at
  WHERE user_id = OLD.user_id
    AND device_id = OLD.target_device_id
    AND approval_status IN ('pending', 'approved')
    AND revoked_at IS NULL;

  INSERT INTO audit_events (
    event_id, user_id, actor_device_id, event_type, subject_type,
    subject_id, outcome, metadata_hash, created_at
  ) VALUES (
    OLD.audit_event_id, OLD.user_id, OLD.approver_device_id, 'device.revoke', 'device',
    OLD.target_device_id, 'success', OLD.request_hash, NEW.completed_at
  );
END;

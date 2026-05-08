CREATE TABLE IF NOT EXISTS user_devices (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  public_key TEXT NOT NULL,
  device_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  approval_status TEXT NOT NULL CHECK (approval_status IN ('pending', 'approved', 'revoked')),
  created_at INTEGER NOT NULL,
  approved_at INTEGER,
  last_active_at INTEGER,
  revoked_at INTEGER,
  idempotency_key TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id),
  UNIQUE (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_user_devices_user_active
  ON user_devices (user_id, revoked_at, last_active_at);

CREATE TABLE IF NOT EXISTS device_approvals (
  user_id TEXT NOT NULL,
  approval_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  requester_device_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
  requested_at INTEGER NOT NULL,
  decided_at INTEGER,
  expires_at INTEGER NOT NULL,
  idempotency_key TEXT NOT NULL,
  PRIMARY KEY (user_id, approval_id),
  UNIQUE (user_id, idempotency_key),
  FOREIGN KEY (user_id, device_id) REFERENCES user_devices (user_id, device_id)
);

CREATE INDEX IF NOT EXISTS idx_device_approvals_user_status
  ON device_approvals (user_id, status, expires_at);

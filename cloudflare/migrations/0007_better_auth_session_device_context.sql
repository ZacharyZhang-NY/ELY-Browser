CREATE TABLE IF NOT EXISTS better_auth_session_device_context (
  session_id TEXT NOT NULL PRIMARY KEY,
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (session_id) REFERENCES better_auth_session (id) ON DELETE CASCADE,
  FOREIGN KEY (user_id, device_id) REFERENCES user_devices (user_id, device_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_better_auth_session_device_context_user_device
  ON better_auth_session_device_context (user_id, device_id);

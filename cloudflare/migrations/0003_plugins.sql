CREATE TABLE IF NOT EXISTS plugin_registry (
  plugin_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  author TEXT NOT NULL,
  permissions_json TEXT NOT NULL,
  signature_state TEXT NOT NULL CHECK (signature_state IN ('valid', 'revoked')),
  min_ely_build TEXT NOT NULL,
  package_r2_key TEXT NOT NULL,
  package_sha256 TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  revoked_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_plugin_registry_signature
  ON plugin_registry (signature_state, updated_at);

CREATE TABLE IF NOT EXISTS plugin_packages (
  plugin_id TEXT NOT NULL,
  package_version TEXT NOT NULL,
  r2_key TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  signature_key_id TEXT NOT NULL,
  signature_value TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
  published_at INTEGER NOT NULL,
  revoked_at INTEGER,
  PRIMARY KEY (plugin_id, package_version),
  FOREIGN KEY (plugin_id) REFERENCES plugin_registry (plugin_id)
);

CREATE INDEX IF NOT EXISTS idx_plugin_packages_active
  ON plugin_packages (plugin_id, revoked_at, published_at);

CREATE TABLE IF NOT EXISTS plugin_reviews (
  review_id TEXT PRIMARY KEY,
  plugin_id TEXT NOT NULL,
  reviewer_user_id TEXT NOT NULL,
  review_status TEXT NOT NULL CHECK (review_status IN ('pending', 'approved', 'rejected')),
  notes_hash TEXT,
  created_at INTEGER NOT NULL,
  decided_at INTEGER,
  FOREIGN KEY (plugin_id) REFERENCES plugin_registry (plugin_id)
);

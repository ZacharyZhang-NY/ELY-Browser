CREATE TABLE IF NOT EXISTS release_manifests (
  channel TEXT NOT NULL CHECK (channel IN ('stable', 'beta', 'nightly')),
  platform TEXT NOT NULL,
  architecture TEXT NOT NULL,
  release_version TEXT NOT NULL,
  artifact_r2_key TEXT NOT NULL,
  artifact_sha256 TEXT NOT NULL,
  artifact_signature TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
  generated_at INTEGER NOT NULL,
  PRIMARY KEY (channel, platform, architecture, release_version)
);

CREATE INDEX IF NOT EXISTS idx_release_manifests_latest
  ON release_manifests (channel, platform, architecture, generated_at);

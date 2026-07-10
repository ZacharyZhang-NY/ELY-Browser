export const CURRENT_SYNC_VAULT_KEY_QUERY = `
  SELECT current_key_id AS key_id, current_generation AS generation
  FROM sync_vault_accounts
  WHERE user_id = ?
`;

export const CURRENT_DEVICE_ENVELOPE_QUERY = `
  SELECT
    accounts.current_key_id AS key_id,
    accounts.current_generation AS generation,
    envelopes.recipient_device_id AS recipient_device_id,
    envelopes.approver_device_id AS approver_device_id,
    envelopes.envelope_version AS envelope_version,
    envelopes.suite AS suite,
    envelopes.encapped_key AS encapped_key,
    envelopes.ciphertext AS ciphertext,
    envelopes.idempotency_key AS idempotency_key,
    envelopes.created_at AS created_at
  FROM sync_vault_accounts AS accounts
  INNER JOIN sync_vault_envelopes AS envelopes
    ON envelopes.user_id = accounts.user_id
    AND envelopes.key_id = accounts.current_key_id
    AND envelopes.generation = accounts.current_generation
  WHERE accounts.user_id = ? AND envelopes.recipient_device_id = ?
`;

export const HISTORICAL_DEVICE_ENVELOPE_QUERY = `
  SELECT
    key_id,
    generation,
    recipient_device_id,
    approver_device_id,
    envelope_version,
    suite,
    encapped_key,
    ciphertext,
    idempotency_key,
    created_at
  FROM sync_vault_envelopes
  WHERE user_id = ? AND recipient_device_id = ? AND key_id = ? AND generation = ?
`;

export const SYNC_VAULT_ACCOUNT_INSERT_QUERY = `
  INSERT INTO sync_vault_accounts (user_id, current_key_id, current_generation, created_at, updated_at)
  SELECT ?, ?, ?, ?, ?
  WHERE EXISTS (
    SELECT 1
    FROM user_devices AS device
    INNER JOIN user_device_keys AS keys
      ON keys.user_id = device.user_id AND keys.device_id = device.device_id
    WHERE device.user_id = ?
      AND device.device_id = ?
      AND device.approval_status = 'approved'
      AND device.revoked_at IS NULL
      AND keys.key_protocol_version = 2
      AND keys.wrapping_public_key IS NOT NULL
  )
  ON CONFLICT(user_id) DO NOTHING
`;

export const SYNC_VAULT_ENVELOPE_INSERT_QUERY = `
  INSERT INTO sync_vault_envelopes (
    user_id, recipient_device_id, approver_device_id, key_id, generation,
    envelope_version, suite, encapped_key, ciphertext, idempotency_key, created_at
  )
  SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
  FROM sync_vault_accounts AS accounts
  INNER JOIN user_devices AS recipient
    ON recipient.user_id = accounts.user_id
    AND recipient.device_id = ?
    AND recipient.approval_status = ?
    AND recipient.revoked_at IS NULL
  INNER JOIN user_device_keys AS recipient_keys
    ON recipient_keys.user_id = recipient.user_id
    AND recipient_keys.device_id = recipient.device_id
    AND recipient_keys.key_protocol_version = 2
    AND recipient_keys.wrapping_public_key IS NOT NULL
  INNER JOIN user_devices AS approver
    ON approver.user_id = accounts.user_id
    AND approver.device_id = ?
    AND approver.approval_status = 'approved'
    AND approver.revoked_at IS NULL
  INNER JOIN user_device_keys AS approver_keys
    ON approver_keys.user_id = approver.user_id
    AND approver_keys.device_id = approver.device_id
    AND approver_keys.key_protocol_version = 2
    AND approver_keys.wrapping_public_key IS NOT NULL
  WHERE accounts.user_id = ?
    AND accounts.current_key_id = ?
    AND accounts.current_generation = ?
  ON CONFLICT DO NOTHING
`;

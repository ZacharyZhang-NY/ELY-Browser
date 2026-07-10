import type { Env } from "./bindings.js";
import { verifyEd25519Signature } from "./device_crypto.js";
import type { WrappedAccountKeyDocument } from "./sync_vault.js";

const BOOTSTRAP_DOMAIN = "elydora-sync-vault-bootstrap-v2";
const PUBLIC_KEY_PATTERN = /^[a-f0-9]{64}$/;

const APPROVED_V2_SIGNING_KEY_QUERY = `
  SELECT keys.signing_public_key
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.signing_public_key IS NOT NULL
`;

export interface SyncVaultBootstrapProofInput {
  keyId: string;
  generation: number;
  envelope: WrappedAccountKeyDocument;
  idempotencyKey: string;
}

interface SigningKeyRow {
  signing_public_key: unknown;
}

export async function syncVaultBootstrapProofValid(
  env: Env,
  userId: string,
  deviceId: string,
  bootstrap: SyncVaultBootstrapProofInput,
  proof: string,
): Promise<boolean> {
  const row = await env.ELY_DB.prepare(APPROVED_V2_SIGNING_KEY_QUERY)
    .bind(userId, deviceId)
    .first<SigningKeyRow>();
  if (
    row === null ||
    typeof row.signing_public_key !== "string" ||
    !PUBLIC_KEY_PATTERN.test(row.signing_public_key)
  ) {
    return false;
  }
  return verifyEd25519Signature(
    row.signing_public_key,
    proof,
    syncVaultBootstrapProofBytes(userId, deviceId, bootstrap),
  );
}

export function syncVaultBootstrapProofBytes(
  userId: string,
  deviceId: string,
  bootstrap: SyncVaultBootstrapProofInput,
): Uint8Array {
  const values: (number | string)[] = [
    BOOTSTRAP_DOMAIN,
    userId,
    deviceId,
    bootstrap.keyId,
    bootstrap.generation,
    bootstrap.envelope.version,
    bootstrap.envelope.suite,
    bootstrap.envelope.encapped_key,
    bootstrap.envelope.ciphertext,
    bootstrap.idempotencyKey,
  ];
  const encoder = new TextEncoder();
  return encoder.encode(values.map((value) => {
    const text = value.toString();
    return `${encoder.encode(text).byteLength}:${text}`;
  }).join(""));
}

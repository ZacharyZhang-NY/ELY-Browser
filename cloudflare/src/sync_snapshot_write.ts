import type { ElyD1DatabaseSession, Env } from "./bindings.js";
import {
  type SyncR2WriteLease,
  abandonSyncR2Write,
  claimSyncR2SnapshotWrite,
  collectSyncR2Garbage,
} from "./sync_r2_gc.js";
import {
  SyncSnapshotRequestError,
  arrayBufferFromBytes,
  sha256Hex,
} from "./sync_snapshot_codec.js";
import type { SnapshotHeadRefDocument } from "./sync_snapshot_head.js";
import { StorageObjectError, putVerifiedObject, syncSnapshotKey } from "./storage.js";

interface SnapshotStorageWrite {
  r2Key: string;
  payloadHash: string;
  keyId: string;
  vaultGeneration: number;
  headRevision: number;
  baseHead: SnapshotHeadRefDocument | null;
  bytes: ArrayBuffer;
}

export type { SyncR2WriteLease } from "./sync_r2_gc.js";

export function claimSnapshotStorageWrite(
  env: Env,
  database: ElyD1DatabaseSession,
  userId: string,
  deviceId: string,
  upload: SnapshotStorageWrite,
  nowSeconds: number,
): Promise<SyncR2WriteLease> {
  return syncOwnerHash(userId).then((ownerHash) => claimSyncR2SnapshotWrite(
    env,
    {
      userId,
      deviceId,
      r2Key: upload.r2Key,
      ownerHash,
      keyId: upload.keyId,
      generation: upload.vaultGeneration,
      headRevision: upload.headRevision,
      baseHead: upload.baseHead === null ? null : {
        revision: upload.baseHead.revision,
        snapshotId: upload.baseHead.snapshot_id,
        payloadHash: upload.baseHead.payload_hash,
      },
    },
    nowSeconds,
    undefined,
    database,
  ));
}

export async function persistClaimedSnapshot(
  env: Env,
  upload: SnapshotStorageWrite,
): Promise<void> {
  try {
    await putVerifiedObject(
      env.ELY_STORAGE,
      upload.r2Key,
      upload.bytes,
      upload.payloadHash,
      "application/octet-stream",
    );
  } catch (error) {
    if (error instanceof StorageObjectError) throw new SyncSnapshotRequestError(error.message);
    throw error;
  }
}

export async function releaseFailedSnapshotWrite(
  env: Env,
  database: ElyD1DatabaseSession,
  userId: string,
  r2Key: string,
  lease: SyncR2WriteLease,
  nowSeconds: number,
): Promise<void> {
  const ownerHash = await syncOwnerHash(userId);
  await abandonSyncR2Write(
    env,
    userId,
    ownerHash,
    r2Key,
    lease.writeToken,
    nowSeconds,
    database,
  );
  try {
    await collectSyncR2Garbage(env, nowSeconds, { ownerHash, limit: 5, database });
  } catch {
    // The durable candidate remains available to scheduled GC.
  }
}

export async function snapshotStorageKey(
  region: string,
  userId: string,
  snapshotId: string,
  payloadHash: string,
): Promise<string> {
  try {
    return syncSnapshotKey({
      region,
      userHash: await syncOwnerHash(userId),
      snapshotId,
      payloadHash,
    });
  } catch (error) {
    if (error instanceof StorageObjectError) throw new SyncSnapshotRequestError(error.message);
    throw error;
  }
}

function syncOwnerHash(userId: string): Promise<string> {
  return sha256Hex(arrayBufferFromBytes(new TextEncoder().encode(userId)));
}

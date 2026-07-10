import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { describe, it } from "node:test";

import type { ElyR2Bucket, ElyR2PutOptions } from "../src/bindings.js";
import {
  StorageObjectError,
  crashAttachmentKey,
  deleteKnownObject,
  exportObjectKey,
  getVerifiedObject,
  pluginAssetKey,
  pluginPackageKey,
  putVerifiedObject,
  syncPayloadKey,
  syncSnapshotKey,
  userAvatarKey,
} from "../src/storage.js";

const USER_HASH = "a".repeat(64);
const PAYLOAD_HASH = "b".repeat(64);
const PACKAGE_HASH = "c".repeat(64);
const ASSET_HASH = "d".repeat(64);
const AVATAR_HASH = "e".repeat(64);
const ATTACHMENT_HASH = "f".repeat(64);

describe("R2 storage contracts", () => {
  it("builds PRD object keys without sensitive path segments", () => {
    assert.equal(
      syncPayloadKey({
        region: "us-east",
        userHash: USER_HASH,
        objectType: "tabs",
        objectId: "tab-01",
        payloadHash: PAYLOAD_HASH,
      }),
      `sync-payloads/us-east/${USER_HASH}/tabs/tab-01/${PAYLOAD_HASH}.bin`,
    );
    assert.equal(
      syncSnapshotKey({
        region: "us-east",
        userHash: USER_HASH,
        snapshotId: "snapshot-01",
        payloadHash: PAYLOAD_HASH,
      }),
      `sync-snapshots/us-east/${USER_HASH}/snapshot-01/${PAYLOAD_HASH}.bin`,
    );
    assert.equal(
      pluginPackageKey({ pluginId: "elydora.reader", packageHash: PACKAGE_HASH }),
      `plugin-packages/elydora.reader/${PACKAGE_HASH}.rplug`,
    );
    assert.equal(
      pluginAssetKey({ pluginId: "elydora.reader", assetHash: ASSET_HASH }),
      `plugin-assets/elydora.reader/${ASSET_HASH}`,
    );
    assert.equal(
      userAvatarKey({ userHash: USER_HASH, avatarHash: AVATAR_HASH }),
      `user-avatars/${USER_HASH}/${AVATAR_HASH}`,
    );
    assert.equal(
      crashAttachmentKey({ reportId: "report-01", attachmentHash: ATTACHMENT_HASH }),
      `crash-attachments/report-01/${ATTACHMENT_HASH}`,
    );
    assert.equal(
      exportObjectKey({ userHash: USER_HASH, exportId: "export-01" }),
      `exports/${USER_HASH}/export-01.bin`,
    );
  });

  it("rejects R2 keys with path traversal or unsupported sync object types", () => {
    assert.throws(
      () =>
        syncPayloadKey({
          region: "us-east",
          userHash: USER_HASH,
          objectType: "passwords",
          objectId: "tab-01",
          payloadHash: PAYLOAD_HASH,
        }),
      StorageObjectError,
    );
    assert.throws(
      () => crashAttachmentKey({ reportId: "../report-01", attachmentHash: ATTACHMENT_HASH }),
      StorageObjectError,
    );
  });

  it("rejects raw sync payload keys with unsupported object types before R2 writes", async () => {
    const bucket = recordedR2Bucket();
    const payload = bytes("encrypted sync payload");
    const checksum = sha256(payload);
    const key = `sync-payloads/us-east/${USER_HASH}/passwords/item-01/${checksum}.bin`;

    await assert.rejects(
      () => putVerifiedObject(bucket, key, payload, checksum, "application/octet-stream"),
      StorageObjectError,
    );
    assert.equal(bucket.puts.length, 0);
  });

  it("writes R2 objects only after checksum verification", async () => {
    const bucket = recordedR2Bucket();
    const payload = bytes("encrypted sync payload");
    const checksum = sha256(payload);
    const key = syncPayloadKey({
      region: "us-east",
      userHash: USER_HASH,
      objectType: "tabs",
      objectId: "tab-01",
      payloadHash: checksum,
    });

    const stored = await putVerifiedObject(bucket, key, payload, checksum, "application/octet-stream");

    assert.deepEqual(stored, { key, sha256: checksum, sizeBytes: payload.byteLength });
    assert.equal(bucket.puts.length, 1);
    assert.equal(bucket.puts[0]?.key, key);
    assert.equal(bucket.puts[0]?.options.httpMetadata?.contentType, "application/octet-stream");
    assert.equal(bucket.puts[0]?.options.customMetadata?.sha256, checksum);
    assert.deepEqual(
      new Uint8Array(bucket.puts[0]?.options.sha256 ?? new ArrayBuffer(0)),
      new Uint8Array(Buffer.from(checksum, "hex")),
    );
  });

  it("rejects payload checksums that disagree with hashed R2 key segments", async () => {
    const bucket = recordedR2Bucket();
    const payload = bytes("encrypted sync payload");
    const checksum = sha256(payload);
    const key = syncPayloadKey({
      region: "us-east",
      userHash: USER_HASH,
      objectType: "tabs",
      objectId: "tab-01",
      payloadHash: PAYLOAD_HASH,
    });

    await assert.rejects(
      () => putVerifiedObject(bucket, key, payload, checksum, "application/octet-stream"),
      StorageObjectError,
    );
    assert.equal(bucket.puts.length, 0);
  });

  it("rejects payload checksum mismatches before R2 writes", async () => {
    const bucket = recordedR2Bucket();
    const payload = bytes("encrypted sync payload");
    const key = syncPayloadKey({
      region: "us-east",
      userHash: USER_HASH,
      objectType: "tabs",
      objectId: "tab-01",
      payloadHash: PAYLOAD_HASH,
    });

    await assert.rejects(
      () => putVerifiedObject(bucket, key, payload, PAYLOAD_HASH, "application/octet-stream"),
      StorageObjectError,
    );
    assert.equal(bucket.puts.length, 0);
  });

  it("verifies R2 downloads against the expected checksum", async () => {
    const payload = bytes("encrypted snapshot");
    const checksum = sha256(payload);
    const bucket = recordedR2Bucket(payload);
    const key = syncSnapshotKey({
      region: "us-east",
      userHash: USER_HASH,
      snapshotId: "snapshot-01",
      payloadHash: checksum,
    });

    const downloaded = await getVerifiedObject(bucket, key, checksum);

    assert.deepEqual(new Uint8Array(downloaded ?? new ArrayBuffer(0)), new Uint8Array(payload));
  });

  it("deletes only known object key shapes", async () => {
    const bucket = recordedR2Bucket();
    const key = syncSnapshotKey({
      region: "us-east",
      userHash: USER_HASH,
      snapshotId: "snapshot-01",
      payloadHash: PAYLOAD_HASH,
    });

    await deleteKnownObject(bucket, key);

    assert.deepEqual(bucket.deletes, [key]);
    await assert.rejects(
      () => deleteKnownObject(bucket, "sync-snapshots/../bad.bin"),
      StorageObjectError,
    );
    assert.deepEqual(bucket.deletes, [key]);
  });
});

interface RecordedPut {
  key: string;
  payload: ArrayBuffer;
  options: ElyR2PutOptions;
}

interface RecordedR2Bucket extends ElyR2Bucket {
  deletes: string[];
  puts: RecordedPut[];
}

function recordedR2Bucket(payload?: ArrayBuffer): RecordedR2Bucket {
  const deletes: string[] = [];
  const puts: RecordedPut[] = [];
  return {
    deletes,
    puts,
    get() {
      if (payload === undefined) {
        return Promise.resolve(null);
      }
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(payload);
        },
      });
    },
    put(key: string, value: ArrayBuffer, options: ElyR2PutOptions = {}) {
      puts.push({ key, payload: value, options });
      return Promise.resolve({
        arrayBuffer() {
          return Promise.resolve(value);
        },
      });
    },
    delete(key: string) {
      deletes.push(key);
      return Promise.resolve();
    },
  };
}

function bytes(value: string): ArrayBuffer {
  return new TextEncoder().encode(value).buffer;
}

function sha256(payload: ArrayBuffer): string {
  return createHash("sha256").update(new Uint8Array(payload)).digest("hex");
}

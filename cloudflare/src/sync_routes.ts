import type { Env } from "./bindings.js";
import { withApprovedDeviceApiControls } from "./api_controls.js";
import { jsonResponse } from "./responses.js";
import { SyncRequestError, SyncSchemaError, syncPullDocument } from "./sync_pull.js";
import {
  SyncPushConflictError,
  SyncPushPersistenceError,
  SyncPushRequestError,
  syncPushDocument,
} from "./sync_push.js";
import {
  SyncSnapshotConflictError,
  SyncSnapshotNotFoundError,
  SyncSnapshotPersistenceError,
  SyncSnapshotRequestError,
  syncSnapshotDownloadDocument,
  syncSnapshotUploadDocument,
} from "./sync_snapshot.js";
import { SyncStatusSchemaError, syncStatusDocument } from "./sync_status.js";

export async function handleSyncRoute(
  request: Request,
  env: Env,
  url: URL,
): Promise<Response | null> {
  if (url.pathname === "/api/sync/pull") {
    return handleSyncPull(request, env, url);
  }
  if (url.pathname === "/api/sync/push") {
    return handleSyncPush(request, env);
  }
  if (url.pathname === "/api/sync/snapshot") {
    return handleSyncSnapshot(request, env, url);
  }
  if (url.pathname === "/api/sync/status") {
    return handleSyncStatus(request, env);
  }
  return null;
}

function handleSyncPull(request: Request, env: Env, url: URL): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.pull",
    ["GET"],
    async (context) => {
      try {
        return jsonResponse(await syncPullDocument(url, env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof SyncRequestError) {
          return jsonResponse(
            { error: "invalid_sync_pull" },
            400,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof SyncSchemaError) {
          return jsonResponse(
            { error: "sync_pull_invalid" },
            500,
            { "Cache-Control": "no-store" },
          );
        }
        throw error;
      }
    },
  );
}

function handleSyncPush(request: Request, env: Env): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.push",
    ["POST"],
    async (context) => {
      try {
        return jsonResponse(await syncPushDocument(request, env, context), 201, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof SyncPushRequestError) {
          return jsonResponse(
            { error: "invalid_sync_push" },
            400,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof SyncPushConflictError) {
          return jsonResponse({ error: "sync_conflict" }, 409, { "Cache-Control": "no-store" });
        }
        if (error instanceof SyncPushPersistenceError) {
          return jsonResponse(
            { error: "sync_push_failed" },
            500,
            { "Cache-Control": "no-store" },
          );
        }
        throw error;
      }
    },
  );
}

function handleSyncSnapshot(request: Request, env: Env, url: URL): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.snapshot",
    ["GET", "POST"],
    async (context) => {
      try {
        if (request.method === "POST") {
          return jsonResponse(await syncSnapshotUploadDocument(request, env, context), 201, {
            "Cache-Control": "no-store",
          });
        }
        return jsonResponse(await syncSnapshotDownloadDocument(url, env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof SyncSnapshotRequestError) {
          return jsonResponse(
            { error: "invalid_sync_snapshot" },
            400,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof SyncSnapshotNotFoundError) {
          return jsonResponse(
            { error: "sync_snapshot_not_found" },
            404,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof SyncSnapshotConflictError) {
          return jsonResponse(
            { error: "sync_snapshot_conflict" },
            409,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof SyncSnapshotPersistenceError) {
          return jsonResponse(
            { error: "sync_snapshot_failed" },
            500,
            { "Cache-Control": "no-store" },
          );
        }
        throw error;
      }
    },
  );
}

function handleSyncStatus(request: Request, env: Env): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.status",
    ["GET"],
    async (context) => {
      try {
        return jsonResponse(await syncStatusDocument(env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof SyncStatusSchemaError) {
          return jsonResponse(
            { error: "sync_status_invalid" },
            500,
            { "Cache-Control": "no-store" },
          );
        }
        throw error;
      }
    },
  );
}

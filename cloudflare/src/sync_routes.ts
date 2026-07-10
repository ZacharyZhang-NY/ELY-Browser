import type { Env } from "./bindings.js";
import { withApprovedDeviceApiControls } from "./api_controls.js";
import { DestructiveActionGateError } from "./destructive_action_gate.js";
import { jsonResponse } from "./responses.js";
import {
  RecentDeviceActionPermissionError,
  RecentDeviceActionPersistenceError,
} from "./recent_device_action_proof.js";
import {
  SyncResetPersistenceError,
  SyncResetRequestError,
  syncResetDocument,
} from "./sync_reset.js";
import {
  SyncSnapshotConflictError,
  SyncSnapshotNotFoundError,
  SyncSnapshotPersistenceError,
  SyncSnapshotRequestError,
  syncSnapshotDownloadDocument,
  syncSnapshotUploadDocument,
} from "./sync_snapshot.js";
import { SyncStatusSchemaError, syncStatusDocument } from "./sync_status.js";
import {
  SyncVaultConflictError,
  SyncVaultNotFoundError,
  SyncVaultPermissionError,
  SyncVaultPersistenceError,
  SyncVaultRequestError,
  syncVaultBootstrapDocument,
  syncVaultCurrentDeviceDocument,
} from "./sync_vault.js";

export async function handleSyncRoute(
  request: Request,
  env: Env,
  url: URL,
): Promise<Response | null> {
  if (url.pathname === "/api/sync/pull") {
    return handleRetiredSyncObjectRoute(request, env, "sync.pull", ["GET"]);
  }
  if (url.pathname === "/api/sync/push") {
    return handleRetiredSyncObjectRoute(request, env, "sync.push", ["POST"]);
  }
  if (url.pathname === "/api/sync/snapshot") {
    return handleSyncSnapshot(request, env, url);
  }
  if (url.pathname === "/api/sync/status") {
    return handleSyncStatus(request, env);
  }
  if (url.pathname === "/api/sync/vault/bootstrap") {
    return handleSyncVaultBootstrap(request, env);
  }
  if (url.pathname === "/api/sync/vault") {
    return handleSyncVault(request, env, url);
  }
  if (url.pathname === "/api/sync/reset") {
    return handleSyncReset(request, env);
  }
  return null;
}

function handleSyncVaultBootstrap(request: Request, env: Env): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.vault.bootstrap",
    ["POST"],
    async (context) => {
      try {
        return jsonResponse(await syncVaultBootstrapDocument(request, env, context), 201, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        const response = syncVaultErrorResponse(error);
        if (response !== null) {
          return response;
        }
        throw error;
      }
    },
  );
}

function handleSyncVault(request: Request, env: Env, url: URL): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.vault",
    ["GET"],
    async (context) => {
      try {
        return jsonResponse(await syncVaultCurrentDeviceDocument(url, env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        const response = syncVaultErrorResponse(error);
        if (response !== null) {
          return response;
        }
        throw error;
      }
    },
  );
}

function syncVaultErrorResponse(error: unknown): Response | null {
  if (error instanceof SyncVaultPermissionError) {
    return jsonResponse({ error: "sync_vault_forbidden" }, 403, { "Cache-Control": "no-store" });
  }
  if (error instanceof SyncVaultRequestError) {
    return jsonResponse({ error: "invalid_sync_vault" }, 400, { "Cache-Control": "no-store" });
  }
  if (error instanceof SyncVaultNotFoundError) {
    return jsonResponse({ error: "sync_vault_not_found" }, 404, { "Cache-Control": "no-store" });
  }
  if (error instanceof SyncVaultConflictError) {
    return jsonResponse({ error: "sync_vault_conflict" }, 409, { "Cache-Control": "no-store" });
  }
  if (error instanceof SyncVaultPersistenceError) {
    return jsonResponse({ error: "sync_vault_failed" }, 500, { "Cache-Control": "no-store" });
  }
  return null;
}

function handleRetiredSyncObjectRoute(
  request: Request,
  env: Env,
  route: string,
  allowedMethods: readonly string[],
): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    route,
    allowedMethods,
    async () =>
      jsonResponse({ error: "sync_object_protocol_retired" }, 410, {
        "Cache-Control": "no-store",
      }),
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
          return jsonResponse(error.document(), 409, { "Cache-Control": "no-store" });
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

function handleSyncReset(request: Request, env: Env): Promise<Response> {
  return withApprovedDeviceApiControls(
    request,
    env,
    "sync.reset",
    ["POST"],
    async (context) => {
      try {
        return jsonResponse(await syncResetDocument(request, env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof SyncResetRequestError) {
          return jsonResponse(
            { error: "invalid_sync_reset" },
            400,
            { "Cache-Control": "no-store" },
          );
        }
        if (error instanceof RecentDeviceActionPermissionError) {
          return jsonResponse(
            { error: "sync_reset_forbidden" },
            403,
            { "Cache-Control": "no-store" },
          );
        }
        if (
          error instanceof SyncResetPersistenceError ||
          error instanceof RecentDeviceActionPersistenceError ||
          error instanceof DestructiveActionGateError
        ) {
          return jsonResponse(
            { error: "sync_reset_failed" },
            500,
            { "Cache-Control": "no-store" },
          );
        }
        throw error;
      }
    },
  );
}

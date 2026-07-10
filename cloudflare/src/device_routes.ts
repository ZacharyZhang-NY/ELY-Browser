import type { Env } from "./bindings.js";
import { withAuthenticatedApiControls } from "./api_controls.js";
import { issueDeviceRebindChallenge, rebindDeviceSession } from "./device_rebind.js";
import { approveDeviceDocument } from "./device_approval.js";
import { revokeDeviceDocument } from "./device_revocation.js";
import {
  DeviceConflictError,
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
  deviceListDocument,
  registerDeviceDocument,
} from "./devices.js";
import { jsonResponse } from "./responses.js";

const NO_STORE = { "Cache-Control": "no-store" } as const;

export async function handleDeviceRoute(
  request: Request,
  env: Env,
  url: URL,
): Promise<Response | null> {
  if (url.pathname === "/api/devices") {
    return withAuthenticatedApiControls(request, env, "devices.list", ["GET"], async (context) => {
      try {
        return jsonResponse(await deviceListDocument(env, context), 200, NO_STORE);
      } catch (error) {
        if (error instanceof DeviceSchemaError) {
          return jsonResponse({ error: "devices_invalid" }, 500, NO_STORE);
        }
        throw error;
      }
    });
  }
  if (url.pathname === "/api/devices/register") {
    return withAuthenticatedApiControls(
      request,
      env,
      "devices.register",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await registerDeviceDocument(request, env, context), 201, NO_STORE);
        } catch (error) {
          if (error instanceof DeviceConflictError) {
            return jsonResponse({ error: "device_registration_conflict" }, 409, NO_STORE);
          }
          if (error instanceof DevicePermissionError) {
            return jsonResponse({ error: "device_registration_forbidden" }, 403, NO_STORE);
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse({ error: "invalid_device_registration" }, 400, NO_STORE);
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse({ error: "device_registration_failed" }, 500, NO_STORE);
          }
          throw error;
        }
      },
    );
  }
  if (url.pathname === "/api/devices/rebind/challenge") {
    return withAuthenticatedApiControls(
      request,
      env,
      "devices.rebind_challenge",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await issueDeviceRebindChallenge(request, env, context), 201, NO_STORE);
        } catch (error) {
          return deviceRebindErrorResponse(error, "challenge");
        }
      },
    );
  }
  if (url.pathname === "/api/devices/rebind") {
    return withAuthenticatedApiControls(
      request,
      env,
      "devices.rebind",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await rebindDeviceSession(request, env, context), 200, NO_STORE);
        } catch (error) {
          return deviceRebindErrorResponse(error, "rebind");
        }
      },
    );
  }
  if (url.pathname === "/api/devices/approve") {
    return withAuthenticatedApiControls(
      request,
      env,
      "devices.approve",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await approveDeviceDocument(request, env, context), 200, NO_STORE);
        } catch (error) {
          if (error instanceof DeviceConflictError) {
            return jsonResponse({ error: "device_approval_conflict" }, 409, NO_STORE);
          }
          if (error instanceof DevicePermissionError) {
            return jsonResponse({ error: "device_approval_forbidden" }, 403, NO_STORE);
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse({ error: "invalid_device_approval" }, 400, NO_STORE);
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse({ error: "device_approval_failed" }, 500, NO_STORE);
          }
          throw error;
        }
      },
    );
  }
  if (url.pathname === "/api/devices/revoke") {
    return withAuthenticatedApiControls(
      request,
      env,
      "devices.revoke",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await revokeDeviceDocument(request, env, context), 200, NO_STORE);
        } catch (error) {
          if (error instanceof DeviceConflictError) {
            return jsonResponse({ error: "device_revocation_conflict" }, 409, NO_STORE);
          }
          if (error instanceof DevicePermissionError) {
            return jsonResponse({ error: "device_revocation_forbidden" }, 403, NO_STORE);
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse({ error: "invalid_device_revocation" }, 400, NO_STORE);
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse({ error: "device_revocation_failed" }, 500, NO_STORE);
          }
          throw error;
        }
      },
    );
  }
  return null;
}

function deviceRebindErrorResponse(error: unknown, operation: "challenge" | "rebind"): Response {
  if (error instanceof DeviceConflictError) {
    return jsonResponse({ error: "device_rebind_conflict" }, 409, NO_STORE);
  }
  if (error instanceof DevicePermissionError) {
    return jsonResponse({ error: "device_rebind_forbidden" }, 403, NO_STORE);
  }
  if (error instanceof DeviceSchemaError) {
    return jsonResponse({ error: `invalid_device_${operation}` }, 400, NO_STORE);
  }
  if (error instanceof DevicePersistenceError) {
    return jsonResponse({ error: "device_rebind_failed" }, 500, NO_STORE);
  }
  throw error;
}

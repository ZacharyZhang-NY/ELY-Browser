import type { Env } from "./bindings.js";
import {
  AuthSessionPersistenceError,
  deleteAuthenticatedSession,
} from "./auth.js";
import {
  withAuthenticatedApiControls,
  withPublicApiControls,
} from "./api_controls.js";
import {
  AccountDeletionPersistenceError,
  AccountDeletionRequestError,
  accountDeletionDocument,
} from "./account_deletion.js";
import { handleBetterAuthRoute } from "./better_auth.js";
import { handleDeviceRoute } from "./device_routes.js";
import { DestructiveActionGateError } from "./destructive_action_gate.js";
import {
  RecentDeviceActionPermissionError,
  RecentDeviceActionPersistenceError,
} from "./recent_device_action_proof.js";
import {
  PluginRegistrySchemaError,
  parsePluginRegistryDocument,
  pluginCatalogDocument,
  pluginDetailsDocument,
  pluginPackageDocument,
  pluginRegistryId,
  pluginRegistryKvKey,
} from "./plugin_registry.js";
import {
  ReleaseManifestSchemaError,
  ReleaseSignatureQueryError,
  parseReleaseManifestDocument,
  parseReleaseSignatureQuery,
  releaseManifestKvKey,
  releaseSignatureDocument,
} from "./release_manifests.js";
import {
  SigningKeysSchemaError,
  parsePublicSigningKeysDocument,
  publicSigningKeysKvKey,
} from "./signing_keys.js";
import { handleSyncRoute } from "./sync_routes.js";
import { maintainSyncR2Storage } from "./sync_r2_maintenance.js";
import {
  TelemetrySchemaError,
  telemetryEventAcceptedDocument,
} from "./telemetry.js";
import { jsonResponse } from "./responses.js";

export default {
  fetch(request: Request, env: Env): Promise<Response> {
    return handleRequest(request, env);
  },
  scheduled(controller: { scheduledTime: number }, env: Env): Promise<void> {
    return maintainSyncR2Storage(env, Math.floor(controller.scheduledTime / 1000));
  },
};

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname === "/api/auth" || url.pathname.startsWith("/api/auth/")) {
    return handleBetterAuthRoute(request, env);
  }
  if (url.pathname === "/api/session/logout") {
    return withAuthenticatedApiControls(
      request,
      env,
      "session.logout",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await deleteAuthenticatedSession(request, env, context), 200, {
            "Cache-Control": "no-store",
          });
        } catch (error) {
          if (error instanceof AuthSessionPersistenceError) {
            return jsonResponse(
              { error: "session_logout_failed" },
              500,
              { "Cache-Control": "no-store" },
            );
          }
          throw error;
        }
      },
    );
  }
  const deviceResponse = await handleDeviceRoute(request, env, url);
  if (deviceResponse !== null) {
    return deviceResponse;
  }
  if (url.pathname === "/api/account/delete") {
    return withAuthenticatedApiControls(
      request,
      env,
      "account.delete",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(await accountDeletionDocument(request, env, context), 200, {
            "Cache-Control": "no-store",
          });
        } catch (error) {
          if (error instanceof AccountDeletionRequestError) {
            return jsonResponse(
              { error: "invalid_account_deletion" },
              400,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof RecentDeviceActionPermissionError) {
            return jsonResponse(
              { error: "account_deletion_forbidden" },
              403,
              { "Cache-Control": "no-store" },
            );
          }
          if (
            error instanceof AccountDeletionPersistenceError ||
            error instanceof RecentDeviceActionPersistenceError ||
            error instanceof DestructiveActionGateError
          ) {
            return jsonResponse(
              { error: "account_deletion_failed" },
              500,
              { "Cache-Control": "no-store" },
            );
          }
          throw error;
        }
      },
    );
  }
  const syncResponse = await handleSyncRoute(request, env, url);
  if (syncResponse !== null) {
    return syncResponse;
  }
  if (url.pathname === "/api/plugins/signing-keys") {
    return withPublicApiControls(request, env, "plugins.signing_keys", ["GET"], () =>
      handlePublicSigningKeys(env),
    );
  }
  const pluginRoute = parsePluginRoute(url.pathname);
  if (pluginRoute !== null) {
    return withPublicApiControls(request, env, pluginRoute.auditRoute, ["GET"], () =>
      handlePluginRoute(env, pluginRoute),
    );
  }
  if (url.pathname === "/api/releases/manifest") {
    return withPublicApiControls(request, env, "releases.manifest", ["GET"], () =>
      handleReleaseManifest(env),
    );
  }
  if (url.pathname === "/api/releases/signature") {
    return withPublicApiControls(request, env, "releases.signature", ["GET"], () =>
      handleReleaseSignature(env, url),
    );
  }
  if (url.pathname === "/api/telemetry/events") {
    return withAuthenticatedApiControls(
      request,
      env,
      "telemetry.events",
      ["POST"],
      async (context) => {
        try {
          return jsonResponse(
            await telemetryEventAcceptedDocument(request, env, context),
            202,
            { "Cache-Control": "no-store" },
          );
        } catch (error) {
          if (error instanceof TelemetrySchemaError) {
            return jsonResponse(
              { error: error.code },
              400,
              { "Cache-Control": "no-store" },
            );
          }
          throw error;
        }
      },
    );
  }

  return jsonResponse({ error: "not_found" }, 404);
}

type PluginRoute =
  | { kind: "catalog"; auditRoute: "plugins.catalog" }
  | { kind: "details"; auditRoute: "plugins.details"; pluginId: string }
  | { kind: "package"; auditRoute: "plugins.package"; pluginId: string };

async function handlePublicSigningKeys(env: Env): Promise<Response> {
  const kvKey = publicSigningKeysKvKey(env.ELY_ENVIRONMENT);
  const value = await env.ELY_KV.get(kvKey);
  if (value === null) {
    return jsonResponse({ error: "public_signing_keys_unavailable" }, 503);
  }

  try {
    const document = parsePublicSigningKeysDocument(value);
    return jsonResponse(document, 200, {
      "Cache-Control": "public, max-age=300, stale-while-revalidate=60",
    });
  } catch (error) {
    if (error instanceof SigningKeysSchemaError) {
      return jsonResponse({ error: "public_signing_keys_invalid" }, 500);
    }
    throw error;
  }
}

async function handlePluginRoute(
  env: Env,
  route: PluginRoute,
): Promise<Response> {
  const kvKey = pluginRegistryKvKey(env.ELY_ENVIRONMENT);
  const value = await env.ELY_KV.get(kvKey);
  if (value === null) {
    return jsonResponse({ error: "plugin_registry_unavailable" }, 503);
  }

  try {
    const registry = parsePluginRegistryDocument(value);
    if (route.kind === "catalog") {
      return jsonResponse(pluginCatalogDocument(registry), 200, publicPluginCacheHeaders());
    }

    if (route.kind === "details") {
      const document = pluginDetailsDocument(registry, route.pluginId);
      if (document === null) {
        return jsonResponse({ error: "plugin_not_found" }, 404);
      }
      return jsonResponse(document, 200, publicPluginCacheHeaders());
    }

    const document = pluginPackageDocument(registry, route.pluginId);
    if (document === null) {
      return jsonResponse({ error: "plugin_not_found" }, 404);
    }
    return jsonResponse(document, 200, publicPluginCacheHeaders());
  } catch (error) {
    if (error instanceof PluginRegistrySchemaError) {
      return jsonResponse({ error: "plugin_registry_invalid" }, 500);
    }
    throw error;
  }
}

async function handleReleaseManifest(env: Env): Promise<Response> {
  const kvKey = releaseManifestKvKey(env.ELY_ENVIRONMENT);
  const value = await env.ELY_KV.get(kvKey);
  if (value === null) {
    return jsonResponse({ error: "release_manifest_unavailable" }, 503);
  }

  try {
    const document = parseReleaseManifestDocument(value);
    return jsonResponse(document, 200, {
      "Cache-Control": "public, max-age=120, stale-while-revalidate=60",
    });
  } catch (error) {
    if (error instanceof ReleaseManifestSchemaError) {
      return jsonResponse({ error: "release_manifest_invalid" }, 500);
    }
    throw error;
  }
}

async function handleReleaseSignature(
  env: Env,
  url: URL,
): Promise<Response> {
  let query;
  try {
    query = parseReleaseSignatureQuery(url.searchParams);
  } catch (error) {
    if (error instanceof ReleaseSignatureQueryError) {
      return jsonResponse({ error: "invalid_release_signature_query" }, 400);
    }
    throw error;
  }

  const kvKey = releaseManifestKvKey(env.ELY_ENVIRONMENT);
  const value = await env.ELY_KV.get(kvKey);
  if (value === null) {
    return jsonResponse({ error: "release_manifest_unavailable" }, 503);
  }

  try {
    const manifest = parseReleaseManifestDocument(value);
    const document = releaseSignatureDocument(manifest, query);
    if (document === null) {
      return jsonResponse({ error: "release_signature_not_found" }, 404);
    }
    return jsonResponse(document, 200, {
      "Cache-Control": "public, max-age=120, stale-while-revalidate=60",
    });
  } catch (error) {
    if (error instanceof ReleaseManifestSchemaError) {
      return jsonResponse({ error: "release_manifest_invalid" }, 500);
    }
    throw error;
  }
}

function parsePluginRoute(pathname: string): PluginRoute | null {
  if (pathname === "/api/plugins") {
    return { kind: "catalog", auditRoute: "plugins.catalog" };
  }

  if (!pathname.startsWith("/api/plugins/")) {
    return null;
  }

  const segments = pathname.split("/");
  if (segments.length === 4) {
    const pluginId = pluginRouteId(segments[3]);
    if (pluginId === null) {
      return null;
    }
    return { kind: "details", auditRoute: "plugins.details", pluginId };
  }
  if (segments.length === 5 && segments[4] === "package") {
    const pluginId = pluginRouteId(segments[3]);
    if (pluginId === null) {
      return null;
    }
    return { kind: "package", auditRoute: "plugins.package", pluginId };
  }

  return null;
}

function pluginRouteId(segment: string | undefined): string | null {
  if (segment === undefined) {
    return null;
  }
  try {
    return pluginRegistryId(segment);
  } catch (error) {
    if (error instanceof PluginRegistrySchemaError) {
      return null;
    }
    throw error;
  }
}

function publicPluginCacheHeaders(): Record<string, string> {
  return { "Cache-Control": "public, max-age=300, stale-while-revalidate=60" };
}

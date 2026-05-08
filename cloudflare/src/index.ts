import type { Env } from "./bindings.js";
import { withAuthenticatedApiControls, withPublicApiControls } from "./api_controls.js";
import {
  DevicePermissionError,
  DevicePersistenceError,
  DeviceSchemaError,
  approveDeviceDocument,
  deviceListDocument,
  registerDeviceDocument,
  revokeDeviceDocument,
} from "./devices.js";
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
import { jsonResponse } from "./responses.js";

export default {
  fetch(request: Request, env: Env): Promise<Response> {
    return handleRequest(request, env);
  },
};

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname === "/api/devices") {
    return withAuthenticatedApiControls(request, env, "devices.list", ["GET"], async (context) => {
      try {
        return jsonResponse(await deviceListDocument(env, context), 200, {
          "Cache-Control": "no-store",
        });
      } catch (error) {
        if (error instanceof DeviceSchemaError) {
          return jsonResponse({ error: "devices_invalid" }, 500, { "Cache-Control": "no-store" });
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
          return jsonResponse(await registerDeviceDocument(request, env, context), 201, {
            "Cache-Control": "no-store",
          });
        } catch (error) {
          if (error instanceof DevicePermissionError) {
            return jsonResponse(
              { error: "device_context_mismatch" },
              403,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse(
              { error: "invalid_device_registration" },
              400,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse(
              { error: "device_registration_failed" },
              500,
              { "Cache-Control": "no-store" },
            );
          }
          throw error;
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
          return jsonResponse(await approveDeviceDocument(request, env, context), 200, {
            "Cache-Control": "no-store",
          });
        } catch (error) {
          if (error instanceof DevicePermissionError) {
            return jsonResponse(
              { error: "device_approval_forbidden" },
              403,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse(
              { error: "invalid_device_approval" },
              400,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse(
              { error: "device_approval_failed" },
              500,
              { "Cache-Control": "no-store" },
            );
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
          return jsonResponse(await revokeDeviceDocument(request, env, context), 200, {
            "Cache-Control": "no-store",
          });
        } catch (error) {
          if (error instanceof DevicePermissionError) {
            return jsonResponse(
              { error: "device_revocation_forbidden" },
              403,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DeviceSchemaError) {
            return jsonResponse(
              { error: "invalid_device_revocation" },
              400,
              { "Cache-Control": "no-store" },
            );
          }
          if (error instanceof DevicePersistenceError) {
            return jsonResponse(
              { error: "device_revocation_failed" },
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

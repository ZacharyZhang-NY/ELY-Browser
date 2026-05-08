import type { Env } from "./bindings.js";
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

export default {
  fetch(request: Request, env: Env): Promise<Response> {
    return handleRequest(request, env);
  },
};

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname === "/api/plugins/signing-keys") {
    return handlePublicSigningKeys(request, env);
  }
  const pluginRoute = parsePluginRoute(url.pathname);
  if (pluginRoute !== null) {
    return handlePluginRoute(request, env, pluginRoute);
  }
  if (url.pathname === "/api/releases/manifest") {
    return handleReleaseManifest(request, env);
  }
  if (url.pathname === "/api/releases/signature") {
    return handleReleaseSignature(request, env, url);
  }

  return jsonResponse({ error: "not_found" }, 404);
}

type PluginRoute =
  | { kind: "catalog" }
  | { kind: "details"; pluginId: string }
  | { kind: "package"; pluginId: string };

async function handlePublicSigningKeys(request: Request, env: Env): Promise<Response> {
  if (request.method !== "GET") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { Allow: "GET" });
  }

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
  request: Request,
  env: Env,
  route: PluginRoute,
): Promise<Response> {
  if (request.method !== "GET") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { Allow: "GET" });
  }

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

async function handleReleaseManifest(request: Request, env: Env): Promise<Response> {
  if (request.method !== "GET") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { Allow: "GET" });
  }

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
  request: Request,
  env: Env,
  url: URL,
): Promise<Response> {
  if (request.method !== "GET") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { Allow: "GET" });
  }

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
    return { kind: "catalog" };
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
    return { kind: "details", pluginId };
  }
  if (segments.length === 5 && segments[4] === "package") {
    const pluginId = pluginRouteId(segments[3]);
    if (pluginId === null) {
      return null;
    }
    return { kind: "package", pluginId };
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

function jsonResponse(
  body: unknown,
  status: number,
  headers: Record<string, string> = {},
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "X-Content-Type-Options": "nosniff",
      ...headers,
    },
  });
}

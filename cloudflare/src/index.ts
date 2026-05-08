import type { Env } from "./bindings.js";
import {
  ReleaseManifestSchemaError,
  parseReleaseManifestDocument,
  releaseManifestKvKey,
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
  if (url.pathname === "/api/releases/manifest") {
    return handleReleaseManifest(request, env);
  }

  return jsonResponse({ error: "not_found" }, 404);
}

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

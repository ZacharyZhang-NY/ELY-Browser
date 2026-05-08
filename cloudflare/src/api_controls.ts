import type { Env } from "./bindings.js";
import {
  type AuthContext,
  AuthError,
  AuthSessionCacheSchemaError,
  authenticatedRateLimitKey,
  readAuthContext,
} from "./auth.js";
import { jsonResponse } from "./responses.js";

const RATE_LIMIT_WINDOW_SECONDS = 60;

export type ApiHandler = () => Promise<Response>;
export type AuthenticatedApiHandler = (context: AuthContext) => Promise<Response>;

export async function withPublicApiControls(
  request: Request,
  env: Env,
  route: string,
  allowedMethods: readonly string[],
  handler: ApiHandler,
): Promise<Response> {
  if (!allowedMethods.includes(request.method)) {
    const response = jsonResponse({ error: "method_not_allowed" }, 405, {
      Allow: allowedMethods.join(", "),
    });
    recordApiAuditEvent(request, env, route, response, "method_not_allowed");
    return response;
  }

  const limit = await env.ELY_RATE_LIMITER.limit({
    key: rateLimitKey(env.ELY_ENVIRONMENT, route),
  });
  if (!limit.success) {
    const response = jsonResponse({ error: "rate_limited" }, 429, {
      "Cache-Control": "no-store",
      "Retry-After": RATE_LIMIT_WINDOW_SECONDS.toString(),
    });
    recordApiAuditEvent(request, env, route, response, "rate_limited");
    return response;
  }

  try {
    const response = await handler();
    recordApiAuditEvent(request, env, route, response, "handled");
    return response;
  } catch (error) {
    recordApiAuditEvent(request, env, route, internalErrorResponse(), "exception");
    throw error;
  }
}

export async function withAuthenticatedApiControls(
  request: Request,
  env: Env,
  route: string,
  allowedMethods: readonly string[],
  handler: AuthenticatedApiHandler,
): Promise<Response> {
  if (!allowedMethods.includes(request.method)) {
    const response = methodNotAllowedResponse(allowedMethods);
    recordApiAuditEvent(request, env, route, response, "method_not_allowed");
    return response;
  }

  const limit = await env.ELY_RATE_LIMITER.limit({
    key: await authenticatedRateLimitKey(env.ELY_ENVIRONMENT, route, request),
  });
  if (!limit.success) {
    const response = rateLimitedResponse();
    recordApiAuditEvent(request, env, route, response, "rate_limited");
    return response;
  }

  let context: AuthContext;
  try {
    context = await readAuthContext(request, env);
  } catch (error) {
    if (error instanceof AuthError) {
      const response = authErrorResponse(error);
      recordApiAuditEvent(request, env, route, response, error.code);
      return response;
    }
    if (error instanceof AuthSessionCacheSchemaError) {
      const response = jsonResponse(
        { error: "auth_session_cache_invalid" },
        500,
        { "Cache-Control": "no-store" },
      );
      recordApiAuditEvent(request, env, route, response, "auth_session_cache_invalid");
      return response;
    }
    throw error;
  }

  try {
    const response = await handler(context);
    recordApiAuditEvent(request, env, route, response, "handled", context);
    return response;
  } catch (error) {
    recordApiAuditEvent(request, env, route, internalErrorResponse(), "exception", context);
    throw error;
  }
}

function rateLimitKey(environment: string, route: string): string {
  return `${environment}:${route}`;
}

function recordApiAuditEvent(
  request: Request,
  env: Env,
  route: string,
  response: Response,
  outcome: string,
  context?: AuthContext,
): void {
  const url = new URL(request.url);
  const blobs = [
    route,
    request.method,
    url.pathname,
    outcome,
    request.headers.get("cf-ray") ?? "",
    request.headers.get("user-agent") ?? "",
  ];
  if (context !== undefined) {
    blobs.push(context.userId, context.deviceId ?? "");
  }

  env.ELY_API_AUDIT.writeDataPoint({
    indexes: [env.ELY_ENVIRONMENT],
    blobs,
    doubles: [response.status, Date.now()],
  });
}

function methodNotAllowedResponse(allowedMethods: readonly string[]): Response {
  return jsonResponse({ error: "method_not_allowed" }, 405, {
    Allow: allowedMethods.join(", "),
  });
}

function rateLimitedResponse(): Response {
  return jsonResponse({ error: "rate_limited" }, 429, {
    "Cache-Control": "no-store",
    "Retry-After": RATE_LIMIT_WINDOW_SECONDS.toString(),
  });
}

function authErrorResponse(error: AuthError): Response {
  return jsonResponse({ error: error.code }, 401, {
    "Cache-Control": "no-store",
    "WWW-Authenticate": "Bearer",
  });
}

function internalErrorResponse(): Response {
  return jsonResponse({ error: "internal_error" }, 500, { "Cache-Control": "no-store" });
}

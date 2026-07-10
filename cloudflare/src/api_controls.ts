import type { Env } from "./bindings.js";
import {
  type AuthContext,
  AuthError,
  AuthSessionSchemaError,
  authenticatedRateLimitKey,
  readAuthContext,
} from "./auth.js";
import { requiresEmailDelivery } from "./better_auth.js";
import { jsonResponse } from "./responses.js";

const RATE_LIMIT_WINDOW_SECONDS = 60;

export type ApiHandler = () => Promise<Response>;
export type AuthenticatedApiHandler = (context: AuthContext) => Promise<Response>;

const APPROVED_DEVICE_QUERY = `
  SELECT device.device_id
  FROM user_devices AS device
  INNER JOIN user_device_keys AS keys
    ON keys.user_id = device.user_id AND keys.device_id = device.device_id
  WHERE device.user_id = ? AND device.device_id = ?
    AND device.approval_status = 'approved' AND device.revoked_at IS NULL
    AND keys.key_protocol_version = 2 AND keys.wrapping_public_key IS NOT NULL
`;

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

/// Better Auth routes are unauthenticated, so they limit per client
/// address. Email-delivery routes consume the tight email limiter; every
/// other auth route shares the standard limiter.
export async function withAuthRouteControls(
  request: Request,
  env: Env,
  handler: ApiHandler,
): Promise<Response> {
  const emailDelivery = requiresEmailDelivery(request);
  const route = emailDelivery ? "auth.email" : "auth.route";
  const allowedMethods = ["GET", "POST"];
  if (!allowedMethods.includes(request.method)) {
    const response = methodNotAllowedResponse(allowedMethods);
    recordApiAuditEvent(request, env, route, response, "method_not_allowed");
    return response;
  }

  const limiter = emailDelivery ? env.ELY_AUTH_EMAIL_RATE_LIMITER : env.ELY_RATE_LIMITER;
  const limit = await limiter.limit({
    key: clientRateLimitKey(env.ELY_ENVIRONMENT, route, request),
  });
  if (!limit.success) {
    const response = rateLimitedResponse();
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
    if (error instanceof AuthSessionSchemaError) {
      const response = jsonResponse(
        { error: "auth_session_invalid" },
        500,
        { "Cache-Control": "no-store" },
      );
      recordApiAuditEvent(request, env, route, response, "auth_session_invalid");
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

export async function withApprovedDeviceApiControls(
  request: Request,
  env: Env,
  route: string,
  allowedMethods: readonly string[],
  handler: AuthenticatedApiHandler,
): Promise<Response> {
  return withAuthenticatedApiControls(request, env, route, allowedMethods, async (context) => {
    if (context.deviceId === undefined) {
      return jsonResponse({ error: "device_context_required" }, 403, {
        "Cache-Control": "no-store",
      });
    }

    const row = await env.ELY_DB.prepare(APPROVED_DEVICE_QUERY)
      .bind(context.userId, context.deviceId)
      .first<ApprovedDeviceRow>();
    if (row === null) {
      return jsonResponse({ error: "device_not_approved" }, 403, {
        "Cache-Control": "no-store",
      });
    }

    return handler(context);
  });
}

interface ApprovedDeviceRow {
  device_id: unknown;
}

function rateLimitKey(environment: string, route: string): string {
  return `${environment}:${route}`;
}

function clientRateLimitKey(environment: string, route: string, request: Request): string {
  const client = request.headers.get("cf-connecting-ip") ?? "unknown";
  return `${environment}:${route}:${client}`;
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

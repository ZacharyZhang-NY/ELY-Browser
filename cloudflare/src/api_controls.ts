import type { Env } from "./bindings.js";
import { jsonResponse } from "./responses.js";

const RATE_LIMIT_WINDOW_SECONDS = 60;

export type ApiHandler = () => Promise<Response>;

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

function rateLimitKey(environment: string, route: string): string {
  return `${environment}:${route}`;
}

function recordApiAuditEvent(
  request: Request,
  env: Env,
  route: string,
  response: Response,
  outcome: string,
): void {
  const url = new URL(request.url);
  env.ELY_API_AUDIT.writeDataPoint({
    indexes: [env.ELY_ENVIRONMENT],
    blobs: [
      route,
      request.method,
      url.pathname,
      outcome,
      request.headers.get("cf-ray") ?? "",
      request.headers.get("user-agent") ?? "",
    ],
    doubles: [response.status, Date.now()],
  });
}

function internalErrorResponse(): Response {
  return jsonResponse({ error: "internal_error" }, 500, { "Cache-Control": "no-store" });
}

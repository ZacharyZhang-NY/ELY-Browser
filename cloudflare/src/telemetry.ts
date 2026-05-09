import type { AuthContext } from "./auth.js";
import type { Env } from "./bindings.js";

const EVENT_TYPES = new Set([
  "app_startup",
  "app_crash",
  "webview_crash",
  "sync_error",
  "plugin_crash",
  "update_result",
]);
const OUTCOMES = new Set(["success", "failure"]);
const PLATFORMS = new Set(["macos", "windows", "linux"]);
const ALLOWED_FIELDS = new Set([
  "version",
  "event_type",
  "occurred_at",
  "app_version",
  "platform",
  "outcome",
  "error_code",
  "component",
  "crash_kind",
  "plugin_id",
]);
const SENSITIVE_FIELDS = new Set([
  "url",
  "loaded_url",
  "loadedurl",
  "page_url",
  "pageurl",
  "title",
  "page_title",
  "pagetitle",
  "search_term",
  "searchterm",
  "query",
  "bookmark",
  "bookmark_content",
  "bookmarkcontent",
  "history",
  "history_content",
  "historycontent",
  "note",
  "notes",
  "notes_content",
  "notescontent",
  "cookie",
  "cookies",
  "form",
  "form_input",
  "forminput",
  "input",
  "field_value",
  "fieldvalue",
  "payload",
  "body",
  "page_text",
  "pagetext",
]);
const APP_VERSION_PATTERN = /^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$/;
const CODE_PATTERN = /^[0-9A-Za-z][0-9A-Za-z._:-]{0,79}$/;

type TelemetryRequestBody = Record<string, unknown>;
type TelemetryEventType =
  | "app_startup"
  | "app_crash"
  | "webview_crash"
  | "sync_error"
  | "plugin_crash"
  | "update_result";
type TelemetryOutcome = "success" | "failure";
type TelemetryPlatform = "macos" | "windows" | "linux";

export interface TelemetryAcceptedDocument {
  version: 1;
  accepted: true;
}

interface TelemetryEvent {
  eventType: TelemetryEventType;
  occurredAt: number;
  appVersion: string;
  platform: TelemetryPlatform;
  outcome?: TelemetryOutcome;
  errorCode?: string;
  component?: string;
  crashKind?: string;
  pluginId?: string;
}

export class TelemetrySchemaError extends Error {
  constructor(readonly code: string) {
    super(code);
    this.name = "TelemetrySchemaError";
  }
}

export async function telemetryEventAcceptedDocument(
  request: Request,
  env: Env,
  context: AuthContext,
): Promise<TelemetryAcceptedDocument> {
  const event = await telemetryEvent(request);
  env.ELY_DIAGNOSTICS.writeDataPoint({
    indexes: [env.ELY_ENVIRONMENT],
    blobs: [
      event.eventType,
      event.appVersion,
      event.platform,
      event.outcome ?? "",
      event.errorCode ?? "",
      event.component ?? "",
      event.crashKind ?? "",
      event.pluginId ?? "",
      context.userId,
      context.deviceId ?? "",
    ],
    doubles: [event.occurredAt, Date.now()],
  });
  return { version: 1, accepted: true };
}

async function telemetryEvent(request: Request): Promise<TelemetryEvent> {
  const body = await requestBody(request);
  assertNoSensitiveFields(body);
  assertOnlyFields(body);
  if (body.version !== 1) {
    throw new TelemetrySchemaError("telemetry_version_invalid");
  }

  const event: TelemetryEvent = {
    eventType: eventType(body.event_type),
    occurredAt: integer(body.occurred_at, "telemetry_occurred_at_invalid"),
    appVersion: appVersion(body.app_version),
    platform: platform(body.platform),
  };
  assignOptionalString(event, "outcome", outcome(body.outcome));
  assignOptionalString(event, "errorCode", code(body.error_code, "telemetry_error_code_invalid"));
  assignOptionalString(event, "component", code(body.component, "telemetry_component_invalid"));
  assignOptionalString(event, "crashKind", code(body.crash_kind, "telemetry_crash_kind_invalid"));
  assignOptionalString(event, "pluginId", code(body.plugin_id, "telemetry_plugin_id_invalid"));
  assertRequiredEventFields(event);
  return event;
}

async function requestBody(request: Request): Promise<TelemetryRequestBody> {
  let value: unknown;
  try {
    value = await request.json();
  } catch {
    throw new TelemetrySchemaError("telemetry_json_invalid");
  }
  if (!isRecord(value)) {
    throw new TelemetrySchemaError("telemetry_body_invalid");
  }
  return value;
}

function assertNoSensitiveFields(value: unknown): void {
  if (Array.isArray(value)) {
    for (const item of value) {
      assertNoSensitiveFields(item);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  for (const [field, fieldValue] of Object.entries(value)) {
    if (SENSITIVE_FIELDS.has(field.toLowerCase())) {
      throw new TelemetrySchemaError("telemetry_sensitive_field");
    }
    assertNoSensitiveFields(fieldValue);
  }
}

function assertOnlyFields(value: TelemetryRequestBody): void {
  for (const field of Object.keys(value)) {
    if (!ALLOWED_FIELDS.has(field)) {
      throw new TelemetrySchemaError("telemetry_unknown_field");
    }
  }
}

function eventType(value: unknown): TelemetryEventType {
  if (typeof value !== "string" || !EVENT_TYPES.has(value)) {
    throw new TelemetrySchemaError("telemetry_event_type_invalid");
  }
  return value as TelemetryEventType;
}

function platform(value: unknown): TelemetryPlatform {
  if (typeof value !== "string" || !PLATFORMS.has(value)) {
    throw new TelemetrySchemaError("telemetry_platform_invalid");
  }
  return value as TelemetryPlatform;
}

function outcome(value: unknown): TelemetryOutcome | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string" || !OUTCOMES.has(value)) {
    throw new TelemetrySchemaError("telemetry_outcome_invalid");
  }
  return value as TelemetryOutcome;
}

function appVersion(value: unknown): string {
  if (typeof value !== "string" || !APP_VERSION_PATTERN.test(value)) {
    throw new TelemetrySchemaError("telemetry_app_version_invalid");
  }
  return value;
}

function code(value: unknown, errorCode: string): string | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== "string" || !CODE_PATTERN.test(value)) {
    throw new TelemetrySchemaError(errorCode);
  }
  return value;
}

function integer(value: unknown, errorCode: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new TelemetrySchemaError(errorCode);
  }
  return value;
}

function assignOptionalString<T extends object, K extends keyof T>(
  target: T,
  field: K,
  value: T[K] | undefined,
): void {
  if (value !== undefined) {
    target[field] = value;
  }
}

function assertRequiredEventFields(event: TelemetryEvent): void {
  if ((event.eventType === "app_startup" || event.eventType === "update_result")
    && event.outcome === undefined) {
    throw new TelemetrySchemaError("telemetry_outcome_required");
  }
  if (event.eventType === "sync_error" && event.errorCode === undefined) {
    throw new TelemetrySchemaError("telemetry_error_code_required");
  }
  if (event.eventType === "webview_crash" && event.crashKind === undefined) {
    throw new TelemetrySchemaError("telemetry_crash_kind_required");
  }
  if (event.eventType === "plugin_crash" && event.pluginId === undefined) {
    throw new TelemetrySchemaError("telemetry_plugin_id_required");
  }
}

function isRecord(value: unknown): value is TelemetryRequestBody {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

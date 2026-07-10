import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyAnalyticsDataPoint, ElyEmailMessageBuilder, Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";

const CLIENT_IP = "203.0.113.9";

describe("auth route controls", () => {
  it("rate limits OTP email delivery before sending mail", async () => {
    const state = testState({ emailLimitSuccess: false });
    const response = await handleRequest(otpRequest(), testEnv(state));

    assert.equal(response.status, 429);
    assert.equal(response.headers.get("retry-after"), "60");
    assert.deepEqual(await response.json(), { error: "rate_limited" });
    assert.deepEqual(state.emailLimitKeys, [`local:auth.email:${CLIENT_IP}`]);
    assert.deepEqual(state.generalLimitKeys, []);
    assert.deepEqual(state.sentEmails, []);
    assert.deepEqual(state.auditEvents[0]?.blobs?.slice(0, 4), [
      "auth.email",
      "POST",
      "/api/auth/email-otp/send-verification-otp",
      "rate_limited",
    ]);
  });

  it("rate limits general auth traffic with its own key", async () => {
    const state = testState({ generalLimitSuccess: false });
    const response = await handleRequest(sessionRequest(), testEnv(state));

    assert.equal(response.status, 429);
    assert.deepEqual(state.generalLimitKeys, [`local:auth.route:${CLIENT_IP}`]);
    assert.deepEqual(state.emailLimitKeys, []);
    assert.deepEqual(state.auditEvents[0]?.blobs?.slice(0, 4), [
      "auth.route",
      "GET",
      "/api/auth/get-session",
      "rate_limited",
    ]);
  });

  it("delivers OTP mail once the email limiter passes", async () => {
    const state = testState({});
    const response = await handleRequest(otpRequest(), testEnv(state));

    assert.equal(response.status, 200);
    assert.deepEqual(state.emailLimitKeys, [`local:auth.email:${CLIENT_IP}`]);
    assert.deepEqual(state.generalLimitKeys, []);
    assert.equal(state.sentEmails.length, 1);
    assert.deepEqual(state.auditEvents[0]?.blobs?.slice(0, 4), [
      "auth.email",
      "POST",
      "/api/auth/email-otp/send-verification-otp",
      "handled",
    ]);
  });

  it("rejects unsupported auth methods before rate limiting", async () => {
    const state = testState({});
    const response = await handleRequest(
      new Request("https://elydora.test/api/auth/get-session", { method: "DELETE" }),
      testEnv(state),
    );

    assert.equal(response.status, 405);
    assert.deepEqual(state.generalLimitKeys, []);
    assert.deepEqual(state.emailLimitKeys, []);
  });
});

function otpRequest(): Request {
  return new Request("https://elydora.test/api/auth/email-otp/send-verification-otp", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "cf-connecting-ip": CLIENT_IP,
    },
    body: JSON.stringify({ email: "user@example.com", type: "sign-in" }),
  });
}

function sessionRequest(): Request {
  return new Request("https://elydora.test/api/auth/get-session", {
    headers: { "cf-connecting-ip": CLIENT_IP },
  });
}

interface TestState {
  emailLimitSuccess: boolean;
  generalLimitSuccess: boolean;
  emailLimitKeys: string[];
  generalLimitKeys: string[];
  sentEmails: ElyEmailMessageBuilder[];
  auditEvents: ElyAnalyticsDataPoint[];
}

function testState(overrides: Partial<TestState>): TestState {
  return {
    emailLimitSuccess: true,
    generalLimitSuccess: true,
    emailLimitKeys: [],
    generalLimitKeys: [],
    sentEmails: [],
    auditEvents: [],
    ...overrides,
  };
}

function testEnv(state: TestState): Env {
  return {
    ELY_ENVIRONMENT: "local",
    ELY_AUTH_BASE_URL: "https://elydora.test",
    ELY_AUTH_SECRET: "8b7f1d2b0c9a4e3f91d6c0a45e72b8f3",
    ELY_DB: testD1Database(),
    ELY_KV: {
      get() {
        return Promise.resolve(null);
      },
      put() {
        return Promise.resolve();
      },
      delete() {
        return Promise.resolve();
      },
    },
    ELY_STORAGE: {
      get() {
        return Promise.resolve(null);
      },
      put() {
        return Promise.resolve({
          arrayBuffer() {
            return Promise.resolve(new ArrayBuffer(0));
          },
        });
      },
      delete() {
        return Promise.resolve();
      },
    },
    ELY_RATE_LIMITER: {
      limit(options: { key: string }) {
        state.generalLimitKeys.push(options.key);
        return Promise.resolve({ success: state.generalLimitSuccess });
      },
    },
    ELY_AUTH_EMAIL_RATE_LIMITER: {
      limit(options: { key: string }) {
        state.emailLimitKeys.push(options.key);
        return Promise.resolve({ success: state.emailLimitSuccess });
      },
    },
    ELY_API_AUDIT: {
      writeDataPoint(event?: ElyAnalyticsDataPoint): void {
        if (event !== undefined) {
          state.auditEvents.push(event);
        }
      },
    },
    ELY_DIAGNOSTICS: {
      writeDataPoint(): void {},
    },
    SEND_EMAIL: {
      send(message: ElyEmailMessageBuilder) {
        state.sentEmails.push(message);
        return Promise.resolve({ messageId: "test-message-id" });
      },
    },
  };
}

function testD1Database(): Env["ELY_DB"] {
  return {
    prepare() {
      return {
        bind() {
          return this;
        },
        first() {
          return Promise.resolve(null);
        },
        all() {
          return Promise.resolve({ results: [], meta: { changes: 1, last_row_id: 1 } });
        },
        run() {
          return Promise.resolve({ results: [], meta: { changes: 1, last_row_id: 1 } });
        },
        raw() {
          return Promise.resolve([]);
        },
      };
    },
    batch() {
      return Promise.resolve([]);
    },
    exec() {
      return Promise.resolve({});
    },
  };
}

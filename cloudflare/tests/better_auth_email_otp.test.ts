import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ElyEmailMessageBuilder, Env } from "../src/bindings.js";
import { handleRequest } from "../src/index.js";

describe("better auth email otp", () => {
  it("sends OTP through Cloudflare Email Send", async () => {
    const sentEmails: ElyEmailMessageBuilder[] = [];
    const response = await handleRequest(otpRequest("USER@example.com"), testEnv(sentEmails));

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { success: true });
    assert.equal(sentEmails.length, 1);

    const email = sentEmails[0]!;
    assert.equal(email.to, "user@example.com");
    assert.deepEqual(email.from, { email: "browser@elydora.com", name: "ELY Browser" });
    assert.equal(email.subject, "Your ELY Browser sign-in code");
    assert.match(email.text ?? "", /Your ELY Browser sign-in code is \d{6}\./);
    assert.match(email.html ?? "", /<strong[^>]*>\d{6}<\/strong>/);
  });

  it("fails closed when the email send binding is unavailable", async () => {
    const response = await handleRequest(otpRequest("user@example.com"), testEnv(null));

    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: "auth_unconfigured" });
  });
});

function otpRequest(email: string): Request {
  return new Request("https://elydora.test/api/auth/email-otp/send-verification-otp", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email, type: "sign-in" }),
  });
}

function testEnv(sentEmails: ElyEmailMessageBuilder[] | null): Env {
  const env: Env = {
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
      limit() {
        return Promise.resolve({ success: true });
      },
    },
    ELY_API_AUDIT: {
      writeDataPoint(): void {},
    },
    ELY_DIAGNOSTICS: {
      writeDataPoint(): void {},
    },
  };

  if (sentEmails !== null) {
    env.SEND_EMAIL = {
      send(message: ElyEmailMessageBuilder) {
        sentEmails.push(message);
        return Promise.resolve({ messageId: "test-message-id" });
      },
    };
  }

  return env;
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
          return Promise.resolve({});
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

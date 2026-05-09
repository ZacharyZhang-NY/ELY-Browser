import { betterAuth, type BetterAuthOptions } from "better-auth";
import { emailOTP } from "better-auth/plugins/email-otp";

import type { Env } from "./bindings.js";
import { jsonResponse } from "./responses.js";

const APP_NAME = "ELY Browser";
const AUTH_BASE_PATH = "/api/auth";
const AUTH_CALLBACK_URL = "ely://auth/callback";
const OTP_REQUEST_TIMEOUT_MS = 10_000;

type BetterAuthDatabase = NonNullable<BetterAuthOptions["database"]>;
type BetterAuthSocialProviders = NonNullable<BetterAuthOptions["socialProviders"]>;

export async function handleBetterAuthRoute(request: Request, env: Env): Promise<Response> {
  try {
    return createElyAuth(env).handler(request);
  } catch (error) {
    if (error instanceof BetterAuthConfigError) {
      return jsonResponse({ error: "auth_unconfigured" }, 500, {
        "Cache-Control": "no-store",
      });
    }
    throw error;
  }
}

function createElyAuth(env: Env) {
  return betterAuth({
    appName: APP_NAME,
    basePath: AUTH_BASE_PATH,
    baseURL: requiredBinding(env.ELY_AUTH_BASE_URL, "ELY_AUTH_BASE_URL"),
    secret: requiredBinding(env.ELY_AUTH_SECRET, "ELY_AUTH_SECRET"),
    database: env.ELY_DB as BetterAuthDatabase,
    emailAndPassword: {
      enabled: true,
      minPasswordLength: 12,
      maxPasswordLength: 128,
      revokeSessionsOnPasswordReset: true,
    },
    socialProviders: socialProviders(env),
    trustedOrigins: [requiredBinding(env.ELY_AUTH_BASE_URL, "ELY_AUTH_BASE_URL"), AUTH_CALLBACK_URL],
    plugins: [
      emailOTP({
        expiresIn: 300,
        allowedAttempts: 3,
        storeOTP: "encrypted",
        resendStrategy: "rotate",
        sendVerificationOTP: (data) => sendVerificationOtp(env, data),
      }),
    ],
  });
}

function socialProviders(env: Env): BetterAuthSocialProviders {
  const providers: BetterAuthSocialProviders = {};
  const google = bindingPair(env.ELY_AUTH_GOOGLE_CLIENT_ID, env.ELY_AUTH_GOOGLE_CLIENT_SECRET);
  if (google !== null) {
    providers.google = {
      clientId: google.clientId,
      clientSecret: google.clientSecret,
    };
  }
  const github = bindingPair(env.ELY_AUTH_GITHUB_CLIENT_ID, env.ELY_AUTH_GITHUB_CLIENT_SECRET);
  if (github !== null) {
    providers.github = {
      clientId: github.clientId,
      clientSecret: github.clientSecret,
    };
  }
  return providers;
}

function bindingPair(
  clientId: string | undefined,
  clientSecret: string | undefined,
): { clientId: string; clientSecret: string } | null {
  if (!isPresent(clientId) || !isPresent(clientSecret)) {
    return null;
  }
  return { clientId: clientId.trim(), clientSecret: clientSecret.trim() };
}

async function sendVerificationOtp(
  env: Env,
  data: {
    email: string;
    otp: string;
    type: "sign-in" | "email-verification" | "forget-password" | "change-email";
  },
): Promise<void> {
  const endpoint = requiredBinding(env.ELY_AUTH_EMAIL_OTP_ENDPOINT, "ELY_AUTH_EMAIL_OTP_ENDPOINT");
  const token = requiredBinding(env.ELY_AUTH_EMAIL_OTP_TOKEN, "ELY_AUTH_EMAIL_OTP_TOKEN");
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), OTP_REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(data),
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new BetterAuthEmailDeliveryError(response.status);
    }
  } finally {
    clearTimeout(timeout);
  }
}

function requiredBinding(value: string | undefined, name: string): string {
  if (isPresent(value)) {
    return value.trim();
  }
  throw new BetterAuthConfigError(name);
}

function isPresent(value: string | undefined): value is string {
  return value !== undefined && value.trim() !== "";
}

class BetterAuthConfigError extends Error {
  constructor(readonly binding: string) {
    super(`missing auth binding: ${binding}`);
    this.name = "BetterAuthConfigError";
  }
}

class BetterAuthEmailDeliveryError extends Error {
  constructor(readonly status: number) {
    super(`auth otp delivery failed with status ${status}`);
    this.name = "BetterAuthEmailDeliveryError";
  }
}

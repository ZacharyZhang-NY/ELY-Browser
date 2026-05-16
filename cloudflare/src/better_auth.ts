import { betterAuth, type BetterAuthOptions } from "better-auth";
import { emailOTP } from "better-auth/plugins/email-otp";

import type { Env } from "./bindings.js";
import { jsonResponse } from "./responses.js";

const APP_NAME = "ELY Browser";
const AUTH_BASE_PATH = "/api/auth";
const AUTH_CALLBACK_URL = "ely://auth/callback";
const EMAIL_OTP_FROM_ADDRESS = "browser@elydora.com";
const EMAIL_OTP_EXPIRES_IN_SECONDS = 300;

type BetterAuthDatabase = NonNullable<BetterAuthOptions["database"]>;
type BetterAuthSocialProviders = NonNullable<BetterAuthOptions["socialProviders"]>;
type VerificationOtpType = "sign-in" | "email-verification" | "forget-password" | "change-email";
type VerificationOtpData = {
  email: string;
  otp: string;
  type: VerificationOtpType;
};

export async function handleBetterAuthRoute(request: Request, env: Env): Promise<Response> {
  try {
    if (requiresEmailDelivery(request)) {
      requiredEmailBinding(env);
    }
    return await createElyAuth(env).handler(request);
  } catch (error) {
    if (error instanceof BetterAuthConfigError) {
      return jsonResponse({ error: "auth_unconfigured" }, 500, {
        "Cache-Control": "no-store",
      });
    }
    if (error instanceof BetterAuthEmailDeliveryError) {
      return jsonResponse({ error: "auth_otp_delivery_failed" }, 502, {
        "Cache-Control": "no-store",
      });
    }
    throw error;
  }
}

function requiresEmailDelivery(request: Request): boolean {
  if (request.method !== "POST") {
    return false;
  }
  const path = new URL(request.url).pathname;
  return (
    path === `${AUTH_BASE_PATH}/email-otp/send-verification-otp` ||
    path === `${AUTH_BASE_PATH}/email-otp/request-password-reset` ||
    path === `${AUTH_BASE_PATH}/forget-password/email-otp` ||
    path === `${AUTH_BASE_PATH}/email-otp/request-email-change`
  );
}

function createElyAuth(env: Env) {
  return betterAuth({
    appName: APP_NAME,
    basePath: AUTH_BASE_PATH,
    baseURL: requiredBinding(env.ELY_AUTH_BASE_URL, "ELY_AUTH_BASE_URL"),
    secret: requiredBinding(env.ELY_AUTH_SECRET, "ELY_AUTH_SECRET"),
    database: env.ELY_DB as BetterAuthDatabase,
    user: { modelName: "better_auth_user" },
    session: { modelName: "better_auth_session" },
    account: { modelName: "better_auth_account" },
    verification: { modelName: "better_auth_verification" },
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
        expiresIn: EMAIL_OTP_EXPIRES_IN_SECONDS,
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
  data: VerificationOtpData,
): Promise<void> {
  const sender = requiredEmailBinding(env);
  try {
    await sender.send({
      to: data.email,
      from: { email: EMAIL_OTP_FROM_ADDRESS, name: APP_NAME },
      subject: verificationOtpSubject(data.type),
      html: verificationOtpHtml(data),
      text: verificationOtpText(data),
    });
  } catch (error) {
    throw new BetterAuthEmailDeliveryError(error);
  }
}

function verificationOtpSubject(type: VerificationOtpType): string {
  switch (type) {
    case "sign-in":
      return `Your ${APP_NAME} sign-in code`;
    case "email-verification":
      return `Verify your ${APP_NAME} email`;
    case "forget-password":
      return `Reset your ${APP_NAME} password`;
    case "change-email":
      return `Confirm your ${APP_NAME} email change`;
  }
}

function verificationOtpText(data: VerificationOtpData): string {
  return [
    `Your ${APP_NAME} ${verificationOtpAction(data.type)} code is ${data.otp}.`,
    "This code expires in 5 minutes.",
    "You can ignore this email if you did not request this code.",
  ].join("\n");
}

function verificationOtpHtml(data: VerificationOtpData): string {
  const code = escapeHtml(data.otp);
  const action = escapeHtml(verificationOtpAction(data.type));
  return [
    "<!doctype html>",
    '<html lang="en">',
    "<body>",
    `<p>Your ${APP_NAME} ${action} code is:</p>`,
    `<p><strong style="font-size:24px;">${code}</strong></p>`,
    "<p>This code expires in 5 minutes.</p>",
    "<p>You can ignore this email if you did not request this code.</p>",
    "</body>",
    "</html>",
  ].join("");
}

function verificationOtpAction(type: VerificationOtpType): string {
  switch (type) {
    case "sign-in":
      return "sign-in";
    case "email-verification":
      return "email verification";
    case "forget-password":
      return "password reset";
    case "change-email":
      return "email change";
  }
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function requiredEmailBinding(env: Env): NonNullable<Env["SEND_EMAIL"]> {
  if (env.SEND_EMAIL !== undefined) {
    return env.SEND_EMAIL;
  }
  throw new BetterAuthConfigError("SEND_EMAIL");
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
  constructor(cause: unknown) {
    super("auth otp delivery failed", { cause });
    this.name = "BetterAuthEmailDeliveryError";
  }
}

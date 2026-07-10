import { verifyEd25519Signature } from "./device_crypto.js";
import {
  type DeviceRegistrationRequest,
  DevicePermissionError,
} from "./device_schema.js";

const REGISTRATION_DOMAIN = "elydora-device-registration-v2";

export async function assertDeviceRegistrationProof(
  registration: DeviceRegistrationRequest,
): Promise<void> {
  const valid = await verifyEd25519Signature(
    registration.publicKey,
    registration.registrationProof,
    deviceRegistrationProofBytes(registration),
  );
  if (!valid) {
    throw new DevicePermissionError("device_registration_proof_invalid");
  }
}

export function deviceRegistrationProofBytes(
  registration: Omit<DeviceRegistrationRequest, "registrationProof">,
): Uint8Array {
  const encoder = new TextEncoder();
  const payload = [
    REGISTRATION_DOMAIN,
    registration.deviceId,
    registration.publicKey,
    registration.wrappingPublicKey,
    registration.deviceName,
    registration.platform,
    registration.idempotencyKey,
  ]
    .map((value) => `${encoder.encode(value).byteLength}:${value}`)
    .join("");
  return encoder.encode(payload);
}

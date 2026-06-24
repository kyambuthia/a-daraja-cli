import { constants, createPublicKey, publicEncrypt } from "node:crypto";
import { certificatePem } from "./certs.js";
import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";

export function currentTimestamp(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    String(now.getUTCFullYear()) +
    pad(now.getUTCMonth() + 1) +
    pad(now.getUTCDate()) +
    pad(now.getUTCHours()) +
    pad(now.getUTCMinutes()) +
    pad(now.getUTCSeconds())
  );
}

export function generateStkPassword(
  shortcode: string,
  passkey: string,
  timestamp: string
): string {
  return Buffer.from(`${shortcode}${passkey}${timestamp}`).toString("base64");
}

export function normalizePhone(phone: string): string {
  let normalized = phone.trim().replace(/^\+/, "");
  if (normalized.startsWith("0")) {
    normalized = `254${normalized.slice(1)}`;
  }
  return normalized;
}

export function generateSecurityCredential(
  initiatorPassword: string,
  certificatePemText: string
): string {
  const pem = certificatePemText.trim();
  if (!pem.includes("BEGIN")) {
    throw new DarajaError("Failed to parse certificate PEM.");
  }

  let publicKey;
  try {
    publicKey = createPublicKey(pem);
  } catch (error) {
    throw new DarajaError(`Failed to parse certificate PEM: ${error}`);
  }

  let encrypted: Buffer;
  try {
    encrypted = publicEncrypt(
      {
        key: publicKey,
        padding: constants.RSA_PKCS1_PADDING,
      },
      Buffer.from(initiatorPassword, "utf8")
    );
  } catch (error) {
    throw new DarajaError(`RSA encryption failed: ${error}`);
  }

  return encrypted.toString("base64");
}

export async function securityCredentialForEnvironment(
  initiatorPassword: string,
  environment: Environment
): Promise<string> {
  const pem = await certificatePem(environment);
  return generateSecurityCredential(initiatorPassword, pem);
}
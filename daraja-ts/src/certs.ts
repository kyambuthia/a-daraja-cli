import { createHash, X509Certificate } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { darajaConfigDir } from "./config.js";
import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";

export const SANDBOX_CERT_URL =
  "https://raw.githubusercontent.com/safaricom/mpesa-node-library/678e3c33b20c1447c209eb88eded89f1bb751e11/keys/sandbox-cert.cer";
export const SANDBOX_CERT_SHA256 =
  "bc97ac696be7c030d71631508b325fc03b335342d88d5ec466c7ffc368bc4550";

export function certificatePath(environment: Environment): string {
  if (process.env.DARAJA_CERT_PATH) {
    return process.env.DARAJA_CERT_PATH;
  }

  if (environment === "sandbox" && process.env.DARAJA_SANDBOX_CERT_PATH) {
    return process.env.DARAJA_SANDBOX_CERT_PATH;
  }

  const filename = environment === "sandbox" ? "sandbox.cer" : "production.cer";
  return join(darajaConfigDir(), filename);
}

function missingCertificateMessage(
  environment: Environment,
  path: string,
  error: unknown
): string {
  if (environment === "sandbox") {
    return (
      `Sandbox certificate not found at ${path}: ${error}. ` +
      "Run `pesaraja init` to download it, or set DARAJA_SANDBOX_CERT_PATH."
    );
  }

  return (
    `Production certificate not found at ${path}: ${error}. ` +
    "Download the Safaricom API certificate from the Daraja portal and save it there, " +
    "or set DARAJA_CERT_PATH."
  );
}

export async function certificatePem(environment: Environment): Promise<string> {
  const path = certificatePath(environment);
  try {
    return await readFile(path, "utf8");
  } catch (error) {
    throw new DarajaError(missingCertificateMessage(environment, path, error));
  }
}

function validateCertificatePem(pem: string): void {
  const trimmed = pem.trim();
  if (
    !trimmed.includes("BEGIN CERTIFICATE") ||
    !trimmed.includes("END CERTIFICATE")
  ) {
    throw new DarajaError("Downloaded sandbox certificate is not valid PEM.");
  }
  try {
    new X509Certificate(pem);
  } catch {
    throw new DarajaError("Downloaded sandbox certificate is not valid X.509.");
  }
}

/** Download the sandbox API certificate if it is not already on disk. */
export async function ensureSandboxCertificate(): Promise<string> {
  const path = certificatePath("sandbox");
  try {
    await readFile(path, "utf8");
    return path;
  } catch {
    // Download below.
  }

  await mkdir(dirname(path), { recursive: true });

  const response = await fetch(SANDBOX_CERT_URL);
  if (!response.ok) {
    throw new DarajaError(
      `Failed to download sandbox certificate from ${SANDBOX_CERT_URL}: HTTP ${response.status}`
    );
  }

  const pem = await response.text();
  validateCertificatePem(pem);
  const digest = createHash("sha256").update(pem).digest("hex");
  if (digest !== SANDBOX_CERT_SHA256) {
    throw new DarajaError(
      `Downloaded sandbox certificate checksum mismatch (got ${digest}).`
    );
  }
  await writeFile(path, pem, "utf8");
  return path;
}

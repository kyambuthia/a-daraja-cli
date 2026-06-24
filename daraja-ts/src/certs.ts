import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";

export const SANDBOX_CERT_URL =
  "https://raw.githubusercontent.com/safaricom/mpesa-node-library/master/keys/sandbox-cert.cer";

function darajaConfigDir(): string {
  return join(homedir(), ".config", "daraja");
}

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
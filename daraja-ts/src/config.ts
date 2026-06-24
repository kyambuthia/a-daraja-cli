import { createDecipheriv } from "node:crypto";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { DarajaError } from "./error.js";
import type { Credentials, Environment } from "./types.js";

interface PlainCreds {
  consumer_key: string;
  consumer_secret: string;
  passkey: string;
  shortcode: string;
  initiator_name?: string;
  initiator_password?: string;
}

export function environmentFromSdkKey(sdkKey: string): Environment {
  if (sdkKey.startsWith("dsk_test_")) return "sandbox";
  if (sdkKey.startsWith("dsk_live_")) return "production";
  throw new DarajaError(
    "SDK key must start with dsk_test_ or dsk_live_."
  );
}

export function extractKeyBytes(sdkKey: string): Buffer {
  const prefix = sdkKey.startsWith("dsk_test_")
    ? "dsk_test_"
    : sdkKey.startsWith("dsk_live_")
      ? "dsk_live_"
      : null;

  if (!prefix) {
    throw new DarajaError("SDK key must start with dsk_test_ or dsk_live_.");
  }

  const b64 = sdkKey.slice(prefix.length);
  const bytes = Buffer.from(b64, "base64");
  if (bytes.length !== 32) {
    throw new DarajaError("SDK key must decode to exactly 32 bytes.");
  }
  return bytes;
}

export function decryptCredentials(
  encrypted: Buffer,
  keyBytes: Buffer
): Credentials {
  if (encrypted.length < 13) {
    throw new DarajaError("Encrypted credentials data is too short.");
  }

  const nonce = encrypted.subarray(0, 12);
  const ciphertext = encrypted.subarray(12);
  const authTag = ciphertext.subarray(ciphertext.length - 16);
  const data = ciphertext.subarray(0, ciphertext.length - 16);

  const decipher = createDecipheriv("aes-256-gcm", keyBytes, nonce);
  decipher.setAuthTag(authTag);

  let plaintext: Buffer;
  try {
    plaintext = Buffer.concat([decipher.update(data), decipher.final()]);
  } catch {
    throw new DarajaError(
      "Decryption failed (wrong key or tampered credentials file)."
    );
  }

  const plain = JSON.parse(plaintext.toString("utf8")) as PlainCreds;
  return {
    consumerKey: plain.consumer_key,
    consumerSecret: plain.consumer_secret,
    passkey: plain.passkey,
    shortcode: plain.shortcode,
    initiatorName: plain.initiator_name,
    initiatorPassword: plain.initiator_password,
  };
}

export async function loadSdkKey(): Promise<string> {
  if (process.env.DARAJA_KEY) {
    return process.env.DARAJA_KEY;
  }

  const path = join(homedir(), ".config", "daraja", "key");
  try {
    return (await readFile(path, "utf8")).trim();
  } catch (error) {
    throw new DarajaError(
      `DARAJA_KEY is not set and could not read ${path}: ${error}`
    );
  }
}

export async function loadCredentials(sdkKey: string): Promise<Credentials> {
  const keyBytes = extractKeyBytes(sdkKey);
  const path = join(homedir(), ".config", "daraja", "credentials.enc");

  let encrypted: Buffer;
  try {
    encrypted = await readFile(path);
  } catch (error) {
    throw new DarajaError(
      `Failed to read credentials file at ${path}: ${error}`
    );
  }

  return decryptCredentials(encrypted, keyBytes);
}
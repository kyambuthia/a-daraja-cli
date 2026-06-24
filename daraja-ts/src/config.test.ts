import { createCipheriv, randomBytes } from "node:crypto";
import { describe, expect, it } from "vitest";
import {
  decryptCredentials,
  environmentFromSdkKey,
  extractKeyBytes,
} from "./config.js";

function encryptLikeRust(plaintext: object, key: Buffer): Buffer {
  const nonce = randomBytes(12);
  const cipher = createCipheriv("aes-256-gcm", key, nonce);
  const data = Buffer.concat([
    cipher.update(JSON.stringify(plaintext), "utf8"),
    cipher.final(),
  ]);
  const authTag = cipher.getAuthTag();
  return Buffer.concat([nonce, data, authTag]);
}

describe("config", () => {
  it("extracts 32-byte key from sdk key", () => {
    const key = randomBytes(32);
    const sdkKey = `dsk_test_${key.toString("base64")}`;
    expect(extractKeyBytes(sdkKey).equals(key)).toBe(true);
  });

  it("detects environment from sdk key prefix", () => {
    const key = `dsk_test_${"a".repeat(44)}`;
    expect(environmentFromSdkKey(key)).toBe("sandbox");
    expect(environmentFromSdkKey(key.replace("test", "live"))).toBe(
      "production"
    );
  });

  it("round-trips credential decrypt compatible with Rust layout", () => {
    const key = randomBytes(32);
    const encrypted = encryptLikeRust(
      {
        consumer_key: "ck",
        consumer_secret: "cs",
        passkey: "pk",
        shortcode: "174379",
        initiator_name: "apitest",
        initiator_password: "pass",
      },
      key
    );

    const creds = decryptCredentials(encrypted, key);
    expect(creds.consumerKey).toBe("ck");
    expect(creds.shortcode).toBe("174379");
    expect(creds.initiatorName).toBe("apitest");
  });
});
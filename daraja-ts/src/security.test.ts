import {
  constants,
  generateKeyPairSync,
  privateDecrypt,
} from "node:crypto";
import { describe, expect, it } from "vitest";
import {
  currentTimestamp,
  generateSecurityCredential,
  generateStkPassword,
  normalizePhone,
} from "./security.js";

describe("security", () => {
  it("generates deterministic STK password", () => {
    const shortcode = "174379";
    const passkey =
      "bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919";
    const timestamp = "20240101120000";
    const password = generateStkPassword(shortcode, passkey, timestamp);
    const decoded = Buffer.from(password, "base64").toString("utf8");
    expect(decoded).toBe(`${shortcode}${passkey}${timestamp}`);
  });

  it("formats timestamp as 14 digits", () => {
    expect(currentTimestamp()).toMatch(/^\d{14}$/);
  });

  it("normalizes Kenyan phone numbers", () => {
    expect(normalizePhone("0712345678")).toBe("254712345678");
    expect(normalizePhone("+254712345678")).toBe("254712345678");
    expect(normalizePhone("254712345678")).toBe("254712345678");
  });

  it("generates RSA security credential from public key PEM", () => {
    const { publicKey, privateKey } = generateKeyPairSync("rsa", {
      modulusLength: 2048,
    });
    const password = "Safaricom999!*!";
    const credential = generateSecurityCredential(password, publicKey.export({
      type: "spki",
      format: "pem",
    }) as string);

    const decoded = Buffer.from(credential, "base64");
    const decrypted = privateDecrypt(
      { key: privateKey, padding: constants.RSA_PKCS1_PADDING },
      decoded
    );
    expect(decrypted.toString("utf8")).toBe(password);
  });

  it("rejects invalid certificate PEM", () => {
    expect(() => generateSecurityCredential("pass", "not a pem")).toThrow(
      "Failed to parse certificate PEM"
    );
  });
});
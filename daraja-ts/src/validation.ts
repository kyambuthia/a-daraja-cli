import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";

export function validateAmount(amount: number): void {
  if (!Number.isSafeInteger(amount) || amount <= 0) {
    throw new DarajaError("Amount must be a positive whole number of KES.");
  }
}

export function validatePhone(phone: string): string {
  let normalized = phone.trim().replace(/^\+/, "");
  if (normalized.startsWith("0")) normalized = `254${normalized.slice(1)}`;
  if (!/^254[17]\d{8}$/.test(normalized)) {
    throw new DarajaError(
      `Invalid Kenyan mobile number '${phone}': expected 07XXXXXXXX, 01XXXXXXXX, or 254XXXXXXXXX.`
    );
  }
  return normalized;
}

export function validateUrl(label: string, value: string, environment: Environment): void {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new DarajaError(`${label} must be an absolute URL.`);
  }
  if (!parsed.hostname || !["http:", "https:"].includes(parsed.protocol)) {
    throw new DarajaError(`${label} must use http or https and include a host.`);
  }
  if (environment === "production" && parsed.protocol !== "https:") {
    throw new DarajaError(`${label} must use HTTPS in production.`);
  }
  if (parsed.username || parsed.password) {
    throw new DarajaError(`${label} must not contain embedded credentials.`);
  }
}

export function validateRequiredText(label: string, value: string, maxLength: number): void {
  const length = [...value].length;
  if (!value.trim() || length > maxLength) {
    throw new DarajaError(`${label} must contain 1-${maxLength} characters.`);
  }
}

export function validateOptionalText(label: string, value: string, maxLength: number): void {
  if ([...value].length > maxLength) {
    throw new DarajaError(`${label} must contain at most ${maxLength} characters.`);
  }
}

export function validateShortcode(shortcode: string): void {
  if (!/^\d{5,9}$/.test(shortcode)) {
    throw new DarajaError("Shortcode must contain 5-9 digits.");
  }
}

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
import { describe, expect, it } from "vitest";
import { validateAmount, validatePhone, validateShortcode, validateUrl } from "./validation.js";

describe("request validation", () => {
  it.each([0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
    "rejects invalid amount %s",
    (amount) => expect(() => validateAmount(amount)).toThrow("positive whole")
  );

  it("normalizes supported Kenyan phone formats", () => {
    expect(validatePhone("0712345678")).toBe("254712345678");
    expect(validatePhone("+254112345678")).toBe("254112345678");
  });

  it.each(["", "712345678", "254812345678", "25471234abcd"])(
    "rejects invalid phone %s",
    (phone) => expect(() => validatePhone(phone)).toThrow("Invalid Kenyan")
  );

  it("requires HTTPS callback URLs in production", () => {
    expect(() => validateUrl("Callback URL", "http://example.com", "production")).toThrow("HTTPS");
    expect(() => validateUrl("Callback URL", "https://example.com", "production")).not.toThrow();
    expect(() => validateUrl("Callback URL", "http://localhost:8181", "sandbox")).not.toThrow();
  });

  it("validates shortcodes", () => {
    expect(() => validateShortcode("174379")).not.toThrow();
    expect(() => validateShortcode("254712345678")).toThrow("5-9 digits");
  });
});

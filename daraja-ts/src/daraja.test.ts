import { describe, expect, it } from "vitest";
import { Daraja } from "./daraja.js";

describe("Daraja constructors", () => {
  it("constructs directly from externally managed credentials", () => {
    const daraja = Daraja.fromCredentials(
      {
        consumerKey: "consumer-key",
        consumerSecret: "consumer-secret",
        passkey: "passkey",
        shortcode: "174379",
      },
      "production",
      { idempotency: false, requestTimeoutMs: 5_000 }
    );

    expect(daraja.getClient().environment).toBe("production");
    expect(daraja.getClient().credentials.shortcode).toBe("174379");
  });
});

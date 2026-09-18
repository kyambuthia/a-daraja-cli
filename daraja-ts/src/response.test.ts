import { describe, expect, it } from "vitest";
import { DarajaError } from "./error.js";
import {
  apiErrorFromBody,
  parseIdempotentJsonBody,
  parsePlainJsonBody,
} from "./response.js";

describe("response helpers", () => {
  it("parses valid JSON bodies", () => {
    expect(parseIdempotentJsonBody('{"ok":true}')).toEqual({ ok: true });
    expect(parsePlainJsonBody<{ value: number }>('{"value":1}')).toEqual({ value: 1 });
  });

  it("maps invalid idempotent JSON to request-outcome-unknown without the raw body", () => {
    const raw = "{not-json consumerSecret: 'must-not-leak'}";
    const error = (() => {
      try {
        parseIdempotentJsonBody(raw);
      } catch (value) {
        return value;
      }
      throw new Error("expected parse to throw");
    })();
    expect(error).toBeInstanceOf(DarajaError);
    expect((error as DarajaError).kind).toBe("request-outcome-unknown");
    expect(String(error)).toContain("provider returned invalid JSON");
    expect(String(error)).toContain("Reconcile before retrying");
    expect(String(error)).not.toContain("must-not-leak");
  });

  it("maps invalid query JSON to a network error without the raw body", () => {
    const raw = "{not-json consumerSecret: 'must-not-leak'}";
    const error = (() => {
      try {
        parsePlainJsonBody(raw);
      } catch (value) {
        return value;
      }
      throw new Error("expected parse to throw");
    })();
    expect(error).toBeInstanceOf(DarajaError);
    expect((error as DarajaError).kind).toBe("network");
    expect(String(error)).toContain("invalid JSON response");
    expect(String(error)).not.toContain("must-not-leak");
  });

  it("bounds API error fields and redacts unknown body fields", () => {
    const longCode = `${"C".repeat(100)}.001`;
    const longMessage = `Request rejected ${"M".repeat(400)}`;
    const error = apiErrorFromBody(
      400,
      JSON.stringify({
        errorCode: longCode,
        errorMessage: longMessage,
        consumerSecret: "must-not-leak",
      })
    );
    expect(error.kind).toBe("api");
    expect(error.status).toBe(400);
    expect(error.code).toBe(longCode.slice(0, 64));
    expect(String(error)).toContain(longMessage.slice(0, 256));
    expect(String(error)).not.toContain("M".repeat(257));
    expect(String(error)).not.toContain("must-not-leak");
  });

  it("falls back for non-object and non-JSON API bodies without leaking them", () => {
    const fromArray = apiErrorFromBody(500, JSON.stringify(["secret-value"]));
    expect(fromArray.code).toBe("unknown");
    expect(String(fromArray)).toContain("response body omitted");
    expect(String(fromArray)).not.toContain("secret-value");

    const fromText = apiErrorFromBody(502, "upstream secret-value body");
    expect(fromText.status).toBe(502);
    expect(String(fromText)).not.toContain("upstream secret-value body");
  });
});

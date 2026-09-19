import { describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import {
  c2bCallbackDeduplicationKey,
  parseC2bConfirmation,
  parseStkCallback,
  matchStkCallback,
  stkCallbackDeduplicationKey,
  stkCallbackMetadata,
  stkCallbackPaid,
  verifyCallback,
  verifyCallbackWith,
} from "./webhook.js";

describe("webhook", () => {
  const sample = {
    Body: {
      stkCallback: {
        MerchantRequestID: "mr-1",
        CheckoutRequestID: "ws_CO_1",
        ResultCode: 0,
        ResultDesc: "Success",
        CallbackMetadata: {
          Item: [
            { Name: "Amount", Value: 500 },
            { Name: "MpesaReceiptNumber", Value: "QAB123" },
          ],
        },
      },
    },
  };

  it("parses STK callback", () => {
    const callback = parseStkCallback(sample);
    expect(callback.CheckoutRequestID).toBe("ws_CO_1");
  });

  it("detects paid callback", () => {
    expect(stkCallbackPaid(sample.Body.stkCallback)).toBe(true);
    expect(
      stkCallbackPaid({ ...sample.Body.stkCallback, ResultCode: 1032 })
    ).toBe(false);
  });

  it("extracts callback metadata", () => {
    const meta = stkCallbackMetadata(sample.Body.stkCallback);
    expect(meta.Amount).toBe(500);
    expect(meta.MpesaReceiptNumber).toBe("QAB123");
  });

  it("matches successful callbacks to the initiated payment", () => {
    const callback = parseStkCallback(sample);
    const matched = matchStkCallback(callback, {
      checkoutRequestId: "ws_CO_1",
      merchantRequestId: "mr-1",
      amount: 500,
    });
    expect(matched.receiptNumber).toBe("QAB123");
    expect(stkCallbackDeduplicationKey(callback)).toBe("stk:receipt:QAB123");
  });

  it("normalizes numeric-string callback amounts", () => {
    const callback = parseStkCallback({
      ...sample,
      Body: {
        stkCallback: {
          ...sample.Body.stkCallback,
          CallbackMetadata: {
            Item: [
              { Name: "Amount", Value: "500" },
              { Name: "MpesaReceiptNumber", Value: "QAB123" },
            ],
          },
        },
      },
    });
    expect(
      matchStkCallback(callback, { checkoutRequestId: "ws_CO_1", amount: 500 }).amount
    ).toBe(500);
  });

  it("rejects mismatched payment callbacks", () => {
    const callback = parseStkCallback(sample);
    expect(() =>
      matchStkCallback(callback, { checkoutRequestId: "ws_CO_1", amount: 501 })
    ).toThrow("amount does not match");
  });

  it("rejects structurally invalid STK callbacks", () => {
    expect(() => parseStkCallback({ Body: { stkCallback: { CheckoutRequestID: "x" } } })).toThrow(
      "MerchantRequestID"
    );
  });

  it("parses C2B confirmation", () => {
    const confirmation = parseC2bConfirmation({
      TransactionType: "Pay Bill",
      TransID: "QEJ78H8JKR",
      TransTime: "20240101120000",
      TransAmount: "500.00",
      BusinessShortCode: "174379",
      BillRefNumber: "Order-1",
      MSISDN: "254712345678",
    });
    expect(confirmation.TransID).toBe("QEJ78H8JKR");
  });

  it("matches the shared callback fixture", async () => {
    const fixture = JSON.parse(
      await readFile(
        new URL("../../daraja-rs/fixtures/callback-contract.json", import.meta.url),
        "utf8"
      )
    ) as { c2b_confirmation: unknown; stk_callback: unknown };
    const confirmation = parseC2bConfirmation(fixture.c2b_confirmation);
    expect(confirmation.TransID).toBe("QEJ78H8JKR");
    expect(confirmation.BillRefNumber).toBe("ORDER-123");
    expect(c2bCallbackDeduplicationKey(confirmation)).toBe("c2b:transaction:QEJ78H8JKR");
    const callback = parseStkCallback(fixture.stk_callback);
    expect(callback.MerchantRequestID).toBe("mr-1");
    expect(callback.CheckoutRequestID).toBe("ws_CO_1");
    expect(stkCallbackPaid(callback)).toBe(true);
    const matched = matchStkCallback(callback, {
      checkoutRequestId: "ws_CO_1",
      merchantRequestId: "mr-1",
      amount: 500,
    });
    expect(matched.receiptNumber).toBe("QAB123");
    expect(matched.phoneNumber).toBe("254712345678");
    expect(stkCallbackDeduplicationKey(callback)).toBe("stk:receipt:QAB123");
  });

  it("fails closed when callback cryptographic verification is unavailable", () => {
    expect(() => verifyCallback("{}", null)).toThrow("Invalid callback authenticator");
    expect(() => verifyCallback("{}", "sig")).toThrow(
      "cryptographic verification is unavailable"
    );
  });

  it("supports an application-owned callback authenticator", () => {
    expect(() =>
      verifyCallbackWith("trusted", "valid", (body, header) => {
        if (body.toString("utf8") !== "trusted" || header !== "valid") {
          throw new Error("invalid callback");
        }
      })
    ).not.toThrow();
  });

  it("keeps basic callback checks ahead of an application authenticator", () => {
    expect(() => verifyCallbackWith("", "valid", () => undefined)).toThrow(
      "Invalid callback authenticator"
    );
    expect(() => verifyCallbackWith("trusted", "", () => undefined)).toThrow(
      "Invalid callback authenticator"
    );
  });

  it("rejects invalid C2B domain fields", () => {
    expect(() =>
      parseC2bConfirmation({
        TransactionType: "Pay Bill",
        TransID: "QEJ78H8JKR",
        TransTime: "not-a-time",
        TransAmount: "0.00",
        BusinessShortCode: "174379",
        BillRefNumber: "Order-1",
        MSISDN: "254712345678",
      })
    ).toThrow("TransTime");
  });
});

import { describe, expect, it } from "vitest";
import {
  parseC2bConfirmation,
  parseStkCallback,
  stkCallbackMetadata,
  stkCallbackPaid,
  verifyCallback,
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

  it("fails closed when callback cryptographic verification is unavailable", () => {
    expect(() => verifyCallback("{}", null)).toThrow("Invalid callback signature");
    expect(() => verifyCallback("{}", "sig")).toThrow(
      "Callback signature verification is not configured"
    );
  });
});

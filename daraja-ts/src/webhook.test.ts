import { describe, expect, it } from "vitest";
import {
  parseStkCallback,
  stkCallbackMetadata,
  stkCallbackPaid,
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
});
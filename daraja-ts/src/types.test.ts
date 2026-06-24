import { describe, expect, it } from "vitest";
import { b2cCommandValue } from "./types.js";

describe("types", () => {
  it("maps B2C command types to Daraja CommandID values", () => {
    expect(b2cCommandValue("business")).toBe("BusinessPayment");
    expect(b2cCommandValue("salary")).toBe("SalaryPayment");
    expect(b2cCommandValue("promotion")).toBe("PromotionPayment");
  });
});
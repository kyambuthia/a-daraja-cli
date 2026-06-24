/**
 * STK Push callback — paste into your Next.js App Router project.
 *
 * Set callbackUrl to https://your-domain.com/api/mpesa/callback when calling prompt().
 */
import {
  parseStkCallback,
  stkCallbackMetadata,
  stkCallbackPaid,
} from "@pesaraja/sdk";

export async function POST(request: Request) {
  const body = await request.json();
  const callback = parseStkCallback(body);

  if (stkCallbackPaid(callback)) {
    const meta = stkCallbackMetadata(callback);
    const receipt = meta.MpesaReceiptNumber;
    const amount = meta.Amount;

    // TODO: mark order paid using callback.CheckoutRequestID, receipt, amount
    console.log("Payment received", { receipt, amount, callback });
  } else {
    console.log("Payment not completed", callback.ResultDesc);
  }

  return Response.json({ ResultCode: 0, ResultDesc: "Success" });
}
/**
 * C2B confirmation callback — paste into your Next.js App Router project.
 *
 * Register this URL with Daraja via pesaraja register or d.register().
 */
import { parseC2bConfirmation, verifyCallback } from "@pesaraja/sdk";

export async function POST(request: Request) {
  const rawBody = await request.text();
  const signature = request.headers.get("X-Safaricom-Signature");

  verifyCallback(rawBody, signature);
  const confirmation = parseC2bConfirmation(JSON.parse(rawBody));

  // TODO: fulfill order using confirmation.BillRefNumber, confirmation.TransID
  console.log("C2B payment received", confirmation);

  return Response.json({
    ResultCode: 0,
    ResultDesc: "Success",
  });
}
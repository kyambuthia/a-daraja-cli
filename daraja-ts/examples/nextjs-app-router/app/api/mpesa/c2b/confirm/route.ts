/**
 * C2B confirmation callback — paste into your Next.js App Router project.
 *
 * Register this URL with Daraja via pesaraja register or d.register().
 */
import { parseC2bConfirmation, verifyCallback } from "@pesaraja/sdk";

export async function POST(request: Request) {
  const rawBody = await request.text();

  // This intentionally throws until cryptographic callback authentication is
  // implemented. Do not replace it with a header-presence check.
  verifyCallback(rawBody, undefined);
  const confirmation = parseC2bConfirmation(JSON.parse(rawBody));

  // Reconcile authoritative transaction state and atomically reject replay
  // before fulfilling an order.
  console.log("C2B payment received", confirmation);

  return Response.json({
    ResultCode: 0,
    ResultDesc: "Success",
  });
}

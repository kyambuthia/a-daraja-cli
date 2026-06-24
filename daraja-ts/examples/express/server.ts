/**
 * Minimal Express callback server — run with tsx or compile first.
 *
 *   npm install express @pesaraja/sdk
 *   DARAJA_KEY=... npx tsx server.ts
 */
import express from "express";
import {
  Daraja,
  parseC2bConfirmation,
  parseStkCallback,
  stkCallbackPaid,
  stkCallbackMetadata,
  verifyCallback,
} from "@pesaraja/sdk";

const app = express();
const port = Number(process.env.PORT ?? 8181);

app.post(
  "/api/mpesa/callback",
  express.json(),
  (req, res) => {
    try {
      const callback = parseStkCallback(req.body);
      if (stkCallbackPaid(callback)) {
        const meta = stkCallbackMetadata(callback);
        console.log("STK paid", meta.MpesaReceiptNumber, meta.Amount);
      }
      res.json({ ResultCode: 0, ResultDesc: "Success" });
    } catch (error) {
      console.error(error);
      res.status(400).json({ error: String(error) });
    }
  }
);

app.post(
  "/api/mpesa/c2b/confirm",
  express.raw({ type: "application/json" }),
  (req, res) => {
    try {
      const rawBody = req.body.toString("utf8");
      verifyCallback(rawBody, req.header("X-Safaricom-Signature"));
      const confirmation = parseC2bConfirmation(JSON.parse(rawBody));
      console.log("C2B paid", confirmation.TransID, confirmation.TransAmount);
      res.json({ ResultCode: 0, ResultDesc: "Success" });
    } catch (error) {
      console.error(error);
      res.status(400).json({ error: String(error) });
    }
  }
);

app.post("/api/mpesa/prompt", express.json(), async (req, res) => {
  try {
    const d = await Daraja.fromEnv();
    const result = await d.prompt({
      phone: req.body.phone,
      amount: req.body.amount,
      callbackUrl: req.body.callbackUrl,
      reference: req.body.reference,
    });
    res.json(result);
  } catch (error) {
    console.error(error);
    res.status(500).json({ error: String(error) });
  }
});

app.listen(port, () => {
  console.log(`M-Pesa callbacks listening on http://localhost:${port}`);
});
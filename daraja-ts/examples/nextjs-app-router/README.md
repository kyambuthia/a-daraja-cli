# Next.js App Router — M-Pesa callbacks

Copy the routes under `app/api/mpesa/` into your Next.js project.

## Setup

1. Run `pesaraja init` and add `DARAJA_KEY` to `.env.local`
2. Install the SDK from this monorepo:

```bash
npm install file:../../  # path to daraja-ts from your Next.js project
```

3. Prompt a payment from a server action or API route:

```typescript
import { Daraja } from "@pesaraja/sdk";

const d = await Daraja.fromEnv();
await d.prompt({
  phone: "254712345678",
  amount: 500,
  callbackUrl: `${process.env.APP_URL}/api/mpesa/callback`,
  reference: "order-123",
});
```

## Routes

| File | Purpose |
|------|---------|
| `app/api/mpesa/callback/route.ts` | STK Push result (customer entered PIN) |
| `app/api/mpesa/c2b/confirm/route.ts` | PayBill / C2B incoming payment |

## Local development

Safaricom requires a public HTTPS URL. Use a tunnel:

```bash
ngrok http 3000
# Use the ngrok URL as APP_URL and --callback-url
```

No domain purchase required for sandbox testing.
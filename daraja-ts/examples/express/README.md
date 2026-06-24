# Express — M-Pesa callbacks

Minimal server with STK and C2B handlers plus a `/api/mpesa/prompt` helper route.

```bash
npm install express @types/express
# SDK from monorepo: npm install file:../../
DARAJA_KEY=dsk_test_... npx tsx server.ts
```

Expose with ngrok for sandbox:

```bash
ngrok http 8181
pesaraja prompt --phone 2547... --amount 500 --callback-url https://YOUR.ngrok-free.app/api/mpesa/callback
```
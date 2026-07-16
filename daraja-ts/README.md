# @pesaraja/sdk

TypeScript/Node.js SDK for Safaricom Daraja and M-Pesa. This package is part
of the [Pesaraja](https://github.com/kyambuthia/pesaraja) monorepo.

The project is alpha software and is not ready for production money movement.
Callback parsing does not authenticate the callback sender; reconcile payment
state before fulfilling an order.

```ts
import { Daraja } from "@pesaraja/sdk";

const daraja = await Daraja.fromEnv();
await daraja.prompt({
  phone: "254712345678",
  amount: 500,
  callbackUrl: "https://example.com/mpesa/callback",
  idempotencyKey: "order:ORDER-123:stk",
});
```

See the repository [README](https://github.com/kyambuthia/pesaraja#readme),
[API guide](https://github.com/kyambuthia/pesaraja/blob/trunk/API.md),
[architecture](https://github.com/kyambuthia/pesaraja/blob/trunk/docs/ARCHITECTURE.md), and
[usage guide](https://github.com/kyambuthia/pesaraja/blob/trunk/docs/usage.md)
for setup, supported operations, and safety constraints.

Licensed under MIT.

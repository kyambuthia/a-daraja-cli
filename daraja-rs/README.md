# daraja-rs

Rust primitives for Safaricom Daraja and M-Pesa APIs. This crate is part of
the [A Daraja CLI](https://github.com/kyambuthia/a-daraja-cli) monorepo.

The project is alpha software and is not ready for production money movement.
Callback parsing does not authenticate the callback sender; reconcile payment
state before fulfilling an order.

```rust
use daraja_rs::Daraja;

let daraja = Daraja::from_env()?;
let client = daraja.client();
```

See the repository [README](https://github.com/kyambuthia/a-daraja-cli#readme),
[API guide](https://github.com/kyambuthia/a-daraja-cli/blob/trunk/API.md),
[architecture](https://github.com/kyambuthia/a-daraja-cli/blob/trunk/docs/ARCHITECTURE.md), and
[usage guide](https://github.com/kyambuthia/a-daraja-cli/blob/trunk/docs/usage.md)
for supported operations and safety constraints.

Licensed under MIT.

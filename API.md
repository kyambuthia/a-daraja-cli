# Daraja API Reference

## Base URLs

| Environment | URL |
|------------|-----|
| Sandbox    | `https://sandbox.safaricom.co.ke` |
| Production | `https://api.safaricom.co.ke` |

## Authentication (OAuth 2.0)

All APIs require an OAuth 2.0 Bearer token.

```
GET /oauth/v1/generate?grant_type=client_credentials
Authorization: Basic base64(consumer_key:consumer_secret)
```

Response:
```json
{
  "access_token": "...",
  "expires_in": "3599"
}
```

Tokens expire in 3600 seconds. Reuse until expiry.

---

## 1. M-Pesa Express (STK Push) — Lipa Na M-Pesa Online

Pushes a payment prompt to a customer's phone.

```
POST /mpesa/stkpush/v1/processrequest
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "BusinessShortCode": "174379",
  "Password": "<base64(Shortcode+Passkey+Timestamp)>",
  "Timestamp": "20240101120000",
  "TransactionType": "CustomerPayBillOnline",
  "Amount": "100",
  "PartyA": "254712345678",
  "PartyB": "174379",
  "PhoneNumber": "254712345678",
  "CallBackURL": "https://your-server.com/callback",
  "AccountReference": "Order #123",
  "TransactionDesc": "Payment for goods"
}
```

**Password format:** `YYYYMMDDHHmmss`  
**Password generation:** `base64(BusinessShortCode + Passkey + Timestamp)`  
**Note:** Passkey is NOT the consumer secret. It's a separate key from the Daraja portal.

Response:
```json
{
  "MerchantRequestID": "...",
  "CheckoutRequestID": "...",
  "ResponseCode": "0",
  "ResponseDescription": "Success. Request accepted for processing",
  "CustomerMessage": "Success. Request accepted for processing"
}
```

### STK Push Query

Check the status of an STK push.

```
POST /mpesa/stkpush/v1/query
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "BusinessShortCode": "174379",
  "Password": "<base64(Shortcode+Passkey+Timestamp)>",
  "Timestamp": "20240101120000",
  "CheckoutRequestID": "ws_CO_..."
}
```

---

## 2. B2C — Business to Customer Payment

Send money from your business/PayBill to a phone number.

```
POST /mpesa/b2c/v1/paymentrequest
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "InitiatorName": "apitest",
  "SecurityCredential": "<encrypted_password>",
  "CommandID": "BusinessPayment",
  "Amount": "100",
  "PartyA": "600000",
  "PartyB": "254712345678",
  "Remarks": "Salary payment",
  "QueueTimeOutURL": "https://your-server.com/timeout",
  "ResultURL": "https://your-server.com/result",
  "Occasion": "June Salary"
}
```

**CommandID options:**
- `BusinessPayment` — for paybill/till to customer
- `SalaryPayment` — for salary disbursement
- `PromotionPayment` — for promotional payments

**SecurityCredential:** Encrypt the initiator password using the Safaricom public certificate. Then base64-encode the result. This is the most painful part of Daraja.

Steps:
1. Get the Safaricom API certificate (sandbox or production)
2. Encrypt the initiator password string with the certificate's public key (RSA, PKCS#1 v1.5)
3. Base64-encode the encrypted bytes

---

## 3. C2B Register URL

Register your validation and confirmation URLs for C2B transactions.

```
POST /mpesa/c2b/v1/registerurl
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "ShortCode": "600000",
  "ResponseType": "Completed",
  "ConfirmationURL": "https://your-server.com/confirmation",
  "ValidationURL": "https://your-server.com/validation"
}
```

**ResponseType options:** `Completed` or `Cancelled`

### C2B Simulate (Sandbox only)

Test a C2B transaction in sandbox.

```
POST /mpesa/c2b/v1/simulate
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "ShortCode": "600000",
  "CommandID": "CustomerPayBillOnline",
  "Amount": "100",
  "Msisdn": "254708374149",
  "BillRefNumber": "Test"
}
```

---

## 4. Transaction Status

Query the status of any M-Pesa transaction.

```
POST /mpesa/transactionstatus/v1/query
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "Initiator": "apitest",
  "SecurityCredential": "<encrypted_password>",
  "CommandID": "TransactionStatusQuery",
  "TransactionID": "QEJ78H8JKR",
  "PartyA": "600000",
  "IdentifierType": "1",
  "ResultURL": "https://your-server.com/result",
  "QueueTimeOutURL": "https://your-server.com/timeout",
  "Remarks": "Check status",
  "Occasion": "Verification"
}
```

**IdentifierType:** `1` (MSISDN), `2` (Till Number), `4` (Shortcode)

---

## 5. Account Balance

Check your M-Pesa account balance.

```
POST /mpesa/accountbalance/v1/query
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "Initiator": "apitest",
  "SecurityCredential": "<encrypted_password>",
  "CommandID": "AccountBalance",
  "PartyA": "600000",
  "IdentifierType": "4",
  "Remarks": "Balance check",
  "QueueTimeOutURL": "https://your-server.com/timeout",
  "ResultURL": "https://your-server.com/result"
}
```

---

## 6. Reversal

Reverse a completed M-Pesa transaction.

```
POST /mpesa/reversal/v1/request
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "Initiator": "apitest",
  "SecurityCredential": "<encrypted_password>",
  "CommandID": "TransactionReversal",
  "TransactionID": "QEJ78H8JKR",
  "Amount": "100",
  "ReceiverParty": "600000",
  "RecieverIdentifierType": "11",
  "ResultURL": "https://your-server.com/result",
  "QueueTimeOutURL": "https://your-server.com/timeout",
  "Remarks": "Wrong amount",
  "Occasion": "Refund"
}
```

---

## Sandbox Credentials (for testing)

| Field | Value |
|-------|-------|
| Consumer Key | Get from Daraja portal (sandbox) |
| Consumer Secret | Get from Daraja portal (sandbox) |
| Passkey | `bfb279f9aa9bdbcf158e97dd71a467cd2e0c893059b10f78e6b72ada1ed2c919` |
| Shortcode | `174379` |
| Initiator Name | `apitest` |
| Initiator Password | `Safaricom999!\*!` |

## Production Credentials

You need a real Safaricom business account + the Daraja production app approved (Go Live).
The C2B Register URL, B2C, Transaction Status, Account Balance, and Reversal APIs
will only work in production after approval.

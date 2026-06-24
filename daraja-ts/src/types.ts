export type Environment = "sandbox" | "production";

export interface Credentials {
  consumerKey: string;
  consumerSecret: string;
  passkey: string;
  shortcode: string;
  initiatorName?: string;
  initiatorPassword?: string;
}

export type StkTransactionType = "paybill" | "till";

export interface PromptOptions {
  phone: string;
  amount: number;
  description?: string;
  reference?: string;
  callbackUrl: string;
  type?: StkTransactionType;
}

export interface StkPushResponse {
  MerchantRequestID: string;
  CheckoutRequestID: string;
  ResponseCode: string;
  ResponseDescription: string;
  CustomerMessage: string;
}

/** Daraja STK callback body (POST to your callback URL). */
export interface StkCallbackBody {
  Body: {
    stkCallback: {
      MerchantRequestID: string;
      CheckoutRequestID: string;
      ResultCode: number;
      ResultDesc: string;
      CallbackMetadata?: {
        Item: Array<{ Name: string; Value: string | number }>;
      };
    };
  };
}

export function environmentBaseUrl(env: Environment): string {
  return env === "sandbox"
    ? "https://sandbox.safaricom.co.ke"
    : "https://api.safaricom.co.ke";
}

export function stkTransactionTypeValue(type: StkTransactionType): string {
  return type === "till" ? "CustomerBuyGoodsOnline" : "CustomerPayBillOnline";
}
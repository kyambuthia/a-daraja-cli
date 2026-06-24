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

/** Async Daraja API response (B2C, status, balance, reversal). */
export interface ConversationResponse {
  OriginatorConversationID: string;
  ConversationID: string;
  ResponseCode: string;
  ResponseDescription: string;
}

export type B2cCommand = "business" | "salary" | "promotion";

export interface SendOptions {
  phone: string;
  amount: number;
  resultUrl: string;
  timeoutUrl: string;
  remarks?: string;
  occasion?: string;
  type?: B2cCommand;
}

export interface QueryOptions {
  transactionId: string;
  resultUrl: string;
  timeoutUrl: string;
  remarks?: string;
  occasion?: string;
}

export interface BalanceOptions {
  resultUrl: string;
  timeoutUrl: string;
}

export interface RefundOptions {
  transactionId: string;
  amount: number;
  receiverParty: string;
  resultUrl: string;
  timeoutUrl: string;
  receiverIdentifierType?: string;
  remarks?: string;
  occasion?: string;
}

export interface C2bRegisterResponse {
  OriginatorConversationID: string;
  ConversationID: string;
  ResponseDescription: string;
}

export interface RegisterOptions {
  confirmationUrl: string;
  validationUrl: string;
  shortcode?: string;
  responseType?: "Completed" | "Cancelled";
}

export interface SimulateOptions {
  phone: string;
  amount: number;
  reference?: string;
  shortcode?: string;
}

/** C2B confirmation callback from Daraja. */
export interface C2bConfirmation {
  TransactionType: string;
  TransID: string;
  TransTime: string;
  TransAmount: string;
  BusinessShortCode: string;
  BillRefNumber: string;
  MSISDN: string;
  FirstName?: string;
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

export function b2cCommandValue(type: B2cCommand): string {
  switch (type) {
    case "salary":
      return "SalaryPayment";
    case "promotion":
      return "PromotionPayment";
    default:
      return "BusinessPayment";
  }
}
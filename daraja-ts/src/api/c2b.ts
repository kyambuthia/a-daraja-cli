import type { DarajaClient } from "../client.js";
import { normalizePhone } from "../security.js";
import type {
  C2bRegisterResponse,
  RegisterOptions,
  SimulateOptions,
} from "../types.js";

const REGISTER_PATH = "/mpesa/c2b/v1/registerurl";
const SIMULATE_PATH = "/mpesa/c2b/v1/simulate";

export async function registerUrls(
  client: DarajaClient,
  options: RegisterOptions
): Promise<C2bRegisterResponse> {
  const shortcode = options.shortcode ?? client.credentials.shortcode;
  return client.post<C2bRegisterResponse>(REGISTER_PATH, {
    ShortCode: shortcode,
    ResponseType: options.responseType ?? "Completed",
    ConfirmationURL: options.confirmationUrl,
    ValidationURL: options.validationUrl,
  }, { idempotencyKey: options.idempotencyKey });
}

export async function simulateC2b(
  client: DarajaClient,
  options: SimulateOptions
): Promise<C2bRegisterResponse> {
  const shortcode = options.shortcode ?? client.credentials.shortcode;
  const phone = normalizePhone(options.phone);

  return client.post<C2bRegisterResponse>(SIMULATE_PATH, {
    ShortCode: shortcode,
    CommandID: "CustomerPayBillOnline",
    Amount: String(Math.trunc(options.amount)),
    Msisdn: phone,
    BillRefNumber: options.reference ?? "Test",
  }, { idempotencyKey: options.idempotencyKey });
}

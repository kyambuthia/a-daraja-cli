import type { DarajaClient } from "../client.js";
import { DarajaError } from "../error.js";
import type {
  C2bRegisterResponse,
  RegisterOptions,
  SimulateOptions,
} from "../types.js";
import { validateAmount, validatePhone, validateRequiredText, validateShortcode, validateUrl } from "../validation.js";

const REGISTER_PATH = "/mpesa/c2b/v1/registerurl";
const SIMULATE_PATH = "/mpesa/c2b/v1/simulate";

export async function registerUrls(
  client: DarajaClient,
  options: RegisterOptions
): Promise<C2bRegisterResponse> {
  const shortcode = options.shortcode ?? client.credentials.shortcode;
  validateShortcode(shortcode);
  validateUrl("Confirmation URL", options.confirmationUrl, client.environment);
  validateUrl("Validation URL", options.validationUrl, client.environment);
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
  if (client.environment !== "sandbox") {
    throw new DarajaError("C2B simulation is available only in the sandbox environment.");
  }
  const shortcode = options.shortcode ?? client.credentials.shortcode;
  validateShortcode(shortcode);
  validateAmount(options.amount);
  const phone = validatePhone(options.phone);
  validateRequiredText("Bill reference", options.reference ?? "Test", 100);

  return client.post<C2bRegisterResponse>(SIMULATE_PATH, {
    ShortCode: shortcode,
    CommandID: "CustomerPayBillOnline",
    Amount: String(options.amount),
    Msisdn: phone,
    BillRefNumber: options.reference ?? "Test",
  }, { idempotencyKey: options.idempotencyKey });
}

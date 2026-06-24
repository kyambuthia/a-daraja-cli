export class DarajaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DarajaError";
  }
}

export function missingApiKey(): DarajaError {
  return new DarajaError("DARAJA_KEY environment variable is not set.");
}
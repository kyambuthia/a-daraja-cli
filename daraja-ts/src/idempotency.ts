import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { DarajaError } from "./error.js";

const TTL_SECS = 86_400;

type EntryStatus = "pending" | "success" | "failed";

interface CacheEntry {
  status: EntryStatus;
  response?: string;
  createdAt: number;
}

type CacheFile = Record<string, CacheEntry>;

export function idempotencyKeyForBody(body: Buffer | string): string {
  const digest = createHash("sha256").update(body).digest();
  const bytes = digest.subarray(0, 16);
  const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

function defaultCachePath(): string {
  return join(homedir(), ".config", "daraja", "idempotency-ts.json");
}

function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}

export class IdempotencyStore {
  private cache: CacheFile | null = null;

  constructor(private readonly path: string = defaultCachePath()) {}

  private async load(): Promise<CacheFile> {
    if (this.cache) return this.cache;
    try {
      const raw = await readFile(this.path, "utf8");
      this.cache = JSON.parse(raw) as CacheFile;
    } catch {
      this.cache = {};
    }
    return this.cache;
  }

  private async save(cache: CacheFile): Promise<void> {
    await mkdir(dirname(this.path), { recursive: true });
    await writeFile(this.path, JSON.stringify(cache), "utf8");
    this.cache = cache;
  }

  cleanupExpired(cache: CacheFile): void {
    const cutoff = unixNow() - TTL_SECS;
    for (const [key, entry] of Object.entries(cache)) {
      if (entry.createdAt < cutoff) {
        delete cache[key];
      }
    }
  }

  async getStatus(key: string): Promise<CacheEntry | undefined> {
    const cache = await this.load();
    this.cleanupExpired(cache);
    return cache[key];
  }

  async savePending(key: string): Promise<void> {
    const cache = await this.load();
    cache[key] = { status: "pending", createdAt: unixNow() };
    await this.save(cache);
  }

  async saveSuccess(key: string, response: Buffer): Promise<void> {
    const cache = await this.load();
    cache[key] = {
      status: "success",
      response: response.toString("base64"),
      createdAt: unixNow(),
    };
    await this.save(cache);
  }

  async saveFailed(key: string): Promise<void> {
    const cache = await this.load();
    cache[key] = { status: "failed", createdAt: unixNow() };
    await this.save(cache);
  }
}

export async function checkIdempotencyCache(
  store: IdempotencyStore,
  body: Buffer
): Promise<Buffer | null> {
  const key = idempotencyKeyForBody(body);
  const entry = await store.getStatus(key);

  if (entry?.status === "success" && entry.response) {
    return Buffer.from(entry.response, "base64");
  }
  if (entry?.status === "pending") {
    throw new DarajaError(
      "Idempotency conflict: a payment with this key is already in flight."
    );
  }

  await store.savePending(key);
  return null;
}
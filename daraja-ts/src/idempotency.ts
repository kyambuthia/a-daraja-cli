import { createHash, randomUUID } from "node:crypto";
import {
  chmod,
  mkdir,
  open,
  readFile,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";

const TTL_SECS = 86_400;
const LOCK_TIMEOUT_MS = 5_000;
const STALE_LOCK_MS = 30_000;

type EntryStatus = "pending" | "success" | "failed" | "unknown";

export interface CacheEntry {
  status: EntryStatus;
  fingerprint: string;
  response?: string;
  createdAt: number;
}

type CacheFile = Record<string, CacheEntry>;

export type AcquireResult =
  | { kind: "acquired" }
  | { kind: "pending" }
  | { kind: "success"; response: Buffer }
  | { kind: "failed" }
  | { kind: "unknown" }
  | { kind: "fingerprint-mismatch" };

/** Storage contract for shared idempotency backends. */
export interface IdempotencyBackend {
  acquire(key: string, fingerprint: string): Promise<AcquireResult>;
  saveSuccess(key: string, response: Buffer): Promise<void>;
  saveFailed(key: string): Promise<void>;
  saveUnknown(key: string): Promise<void>;
}

export function requestFingerprint(body: Buffer | string): string {
  return createHash("sha256").update(body).digest("hex");
}

export function idempotencyKeyForBody(body: Buffer | string): string {
  const hex = requestFingerprint(body).slice(0, 32);
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

export function scopedIdempotencyKey(
  environment: Environment,
  shortcode: string,
  path: string,
  callerKey: string
): string {
  return requestFingerprint(
    `${environment}\0${shortcode}\0${path}\0${callerKey}`
  );
}

export function validateIdempotencyKey(key: string): void {
  if (!/^[A-Za-z0-9._:-]{1,128}$/.test(key)) {
    throw new DarajaError(
      "Idempotency key must be 1-128 ASCII letters, numbers, '.', ':', '_' or '-'."
    );
  }
}

function defaultCachePath(): string {
  return join(homedir(), ".config", "daraja", "idempotency-ts.json");
}

function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}

function isCacheEntry(value: unknown): value is CacheEntry {
  if (!value || typeof value !== "object") return false;
  const entry = value as Partial<CacheEntry>;
  return (
    ["pending", "success", "failed", "unknown"].includes(entry.status ?? "") &&
    typeof entry.fingerprint === "string" &&
    typeof entry.createdAt === "number" &&
    (entry.response === undefined || typeof entry.response === "string")
  );
}

function parseCache(raw: string): CacheFile {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new DarajaError(
      "Idempotency cache is corrupt; refusing to send a potentially duplicate payment."
    );
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new DarajaError("Idempotency cache has an invalid format.");
  }
  for (const value of Object.values(parsed)) {
    if (!isCacheEntry(value)) {
      throw new DarajaError("Idempotency cache has an invalid entry.");
    }
  }
  return parsed as CacheFile;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export class IdempotencyStore implements IdempotencyBackend {
  constructor(private readonly path: string = defaultCachePath()) {}

  private async loadFresh(): Promise<CacheFile> {
    try {
      return parseCache(await readFile(this.path, "utf8"));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return {};
      if (error instanceof DarajaError) throw error;
      throw new DarajaError(`Failed to read idempotency cache: ${error}`);
    }
  }

  private async saveAtomically(cache: CacheFile): Promise<void> {
    const directory = dirname(this.path);
    await mkdir(directory, { recursive: true, mode: 0o700 });
    await chmod(directory, 0o700).catch(() => undefined);
    const temporary = `${this.path}.${process.pid}.${randomUUID()}.tmp`;
    try {
      await writeFile(temporary, JSON.stringify(cache), {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporary, this.path);
      await chmod(this.path, 0o600).catch(() => undefined);
    } finally {
      await rm(temporary, { force: true }).catch(() => undefined);
    }
  }

  private async withFileLock<T>(operation: () => Promise<T>): Promise<T> {
    const directory = dirname(this.path);
    const lockPath = `${this.path}.lock`;
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const deadline = Date.now() + LOCK_TIMEOUT_MS;
    let lock: Awaited<ReturnType<typeof open>> | undefined;

    while (!lock) {
      try {
        lock = await open(lockPath, "wx", 0o600);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
        const details = await stat(lockPath).catch(() => null);
        if (details && Date.now() - details.mtimeMs > STALE_LOCK_MS) {
          await rm(lockPath, { force: true });
          continue;
        }
        if (Date.now() >= deadline) {
          throw new DarajaError(
            "Timed out waiting for the idempotency cache lock; no request was sent."
          );
        }
        await delay(20);
      }
    }

    try {
      return await operation();
    } finally {
      await lock.close();
      await rm(lockPath, { force: true });
    }
  }

  cleanupExpired(cache: CacheFile): void {
    const cutoff = unixNow() - TTL_SECS;
    for (const [key, entry] of Object.entries(cache)) {
      if (entry.createdAt < cutoff) delete cache[key];
    }
  }

  async acquire(key: string, fingerprint: string): Promise<AcquireResult> {
    return this.withFileLock(async () => {
      const cache = await this.loadFresh();
      this.cleanupExpired(cache);
      const entry = cache[key];
      if (!entry) {
        cache[key] = { status: "pending", fingerprint, createdAt: unixNow() };
        await this.saveAtomically(cache);
        return { kind: "acquired" };
      }
      if (entry.fingerprint !== fingerprint) {
        return { kind: "fingerprint-mismatch" };
      }
      if (entry.status === "success" && entry.response) {
        return { kind: "success", response: Buffer.from(entry.response, "base64") };
      }
      return { kind: entry.status } as AcquireResult;
    });
  }

  async getStatus(key: string): Promise<CacheEntry | undefined> {
    const cache = await this.loadFresh();
    this.cleanupExpired(cache);
    return cache[key];
  }

  async saveSuccess(key: string, response: Buffer): Promise<void> {
    await this.update(key, "success", response.toString("base64"));
  }

  async saveFailed(key: string): Promise<void> {
    await this.update(key, "failed");
  }

  async saveUnknown(key: string): Promise<void> {
    await this.update(key, "unknown");
  }

  private async update(
    key: string,
    status: EntryStatus,
    response?: string
  ): Promise<void> {
    await this.withFileLock(async () => {
      const cache = await this.loadFresh();
      const entry = cache[key];
      if (!entry) {
        throw new DarajaError("Idempotency reservation was lost; do not retry.");
      }
      cache[key] = { ...entry, status, response, createdAt: unixNow() };
      await this.saveAtomically(cache);
    });
  }
}

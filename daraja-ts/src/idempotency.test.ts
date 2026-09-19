import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  IdempotencyStore,
  requestFingerprint,
  validateIdempotencyKey,
} from "./idempotency.js";

describe("idempotency", () => {
  let tempDir = "";

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
      tempDir = "";
    }
  });

  async function tempStore(): Promise<IdempotencyStore> {
    tempDir = await mkdtemp(join(tmpdir(), "daraja-idem-"));
    return new IdempotencyStore(join(tempDir, "cache.json"));
  }

  it("validates stable caller-provided keys", () => {
    expect(() => validateIdempotencyKey("checkout:order-123")).not.toThrow();
    expect(() => validateIdempotencyKey("contains spaces")).toThrow("1-128");
  });

  it("returns cached success response", async () => {
    const store = await tempStore();
    const body = Buffer.from('{"test":true}');
    const key = "test:cached-success";
    const fingerprint = requestFingerprint(body);

    expect(await store.acquire(key, fingerprint)).toEqual({ kind: "acquired" });
    await store.saveSuccess(key, Buffer.from('{"ok":true}'));

    const cached = await store.acquire(key, fingerprint);
    expect(cached.kind).toBe("success");
    if (cached.kind === "success") {
      expect(cached.response.toString("utf8")).toBe('{"ok":true}');
    }
  });

  it("atomically rejects a duplicate in-flight request", async () => {
    const store = await tempStore();
    const body = Buffer.from('{"pending":true}');
    const key = "test:in-flight";
    const fingerprint = requestFingerprint(body);

    const results = await Promise.all([
      store.acquire(key, fingerprint),
      store.acquire(key, fingerprint),
    ]);
    expect(results.map((result) => result.kind).sort()).toEqual([
      "acquired",
      "pending",
    ]);
  });

  it("rejects key reuse with a different payload", async () => {
    const store = await tempStore();
    expect(await store.acquire("order-1", "first")).toEqual({ kind: "acquired" });
    expect(await store.acquire("order-1", "second")).toEqual({
      kind: "fingerprint-mismatch",
    });
  });

  it("fails closed when the cache is corrupt", async () => {
    tempDir = await mkdtemp(join(tmpdir(), "daraja-idem-"));
    const path = join(tempDir, "cache.json");
    await writeFile(path, "not-json", "utf8");
    const store = new IdempotencyStore(path);

    await expect(store.acquire("order-1", "fingerprint")).rejects.toThrow(
      "cache is corrupt"
    );
  });

  it("preserves old pending and unknown outcomes during cleanup", async () => {
    tempDir = await mkdtemp(join(tmpdir(), "daraja-idem-"));
    const path = join(tempDir, "cache.json");
    await writeFile(
      path,
      JSON.stringify({
        pending: { status: "pending", fingerprint: "p", createdAt: 0 },
        unknown: { status: "unknown", fingerprint: "u", createdAt: 0 },
        success: {
          status: "success",
          fingerprint: "s",
          response: Buffer.from("{}").toString("base64"),
          createdAt: 0,
        },
      }),
      "utf8"
    );
    const store = new IdempotencyStore(path);

    expect(await store.acquire("pending", "p")).toEqual({ kind: "pending" });
    expect(await store.acquire("unknown", "u")).toEqual({ kind: "unknown" });
    expect(await store.acquire("success", "s")).toEqual({ kind: "acquired" });
  });
});

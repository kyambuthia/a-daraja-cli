import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  idempotencyKeyForBody,
  IdempotencyStore,
  requestFingerprint,
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

  it("derives deterministic key from request body", () => {
    const body = Buffer.from('{"Amount":"100"}');
    const key1 = idempotencyKeyForBody(body);
    const key2 = idempotencyKeyForBody(body);
    expect(key1).toBe(key2);
    expect(key1).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
    );
  });

  it("returns cached success response", async () => {
    const store = await tempStore();
    const body = Buffer.from('{"test":true}');
    const key = idempotencyKeyForBody(body);
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
    const key = idempotencyKeyForBody(body);
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
});

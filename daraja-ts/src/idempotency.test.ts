import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  checkIdempotencyCache,
  idempotencyKeyForBody,
  IdempotencyStore,
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

    await store.savePending(key);
    await store.saveSuccess(key, Buffer.from('{"ok":true}'));

    const cached = await checkIdempotencyCache(store, body);
    expect(cached?.toString("utf8")).toBe('{"ok":true}');
  });

  it("rejects duplicate in-flight request", async () => {
    const store = await tempStore();
    const body = Buffer.from('{"pending":true}');

    await checkIdempotencyCache(store, body);
    await expect(checkIdempotencyCache(store, body)).rejects.toThrow(
      "Idempotency conflict"
    );
  });
});
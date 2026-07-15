import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DarajaClient } from "./client.js";
import { IdempotencyStore } from "./idempotency.js";
import type { Credentials } from "./types.js";
import { simulateC2b } from "./api/c2b.js";

const credentials: Credentials = {
  consumerKey: "consumer-key",
  consumerSecret: "consumer-secret",
  passkey: "passkey",
  shortcode: "174379",
};

describe("DarajaClient idempotency", () => {
  let tempDir = "";

  afterEach(async () => {
    vi.unstubAllGlobals();
    if (tempDir) await rm(tempDir, { recursive: true, force: true });
    tempDir = "";
  });

  it("sends a caller-supplied idempotency key", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockResolvedValueOnce(new Response(JSON.stringify({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotency: false,
    });

    await client.post(
      "/payment",
      { Amount: "100" },
      { idempotencyKey: "order-123" }
    );

    expect(fetchMock).toHaveBeenCalledTimes(2);
    const request = fetchMock.mock.calls[1]?.[1] as RequestInit;
    expect(request.headers).toMatchObject({ "Idempotency-Key": "order-123" });
  });

  it("rejects the same key with different request parameters", async () => {
    tempDir = await mkdtemp(join(tmpdir(), "daraja-client-"));
    const store = new IdempotencyStore(join(tempDir, "idempotency.json"));
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockResolvedValueOnce(new Response(JSON.stringify({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotencyStore: store,
    });

    await client.post(
      "/payment",
      { Amount: "100" },
      { idempotencyKey: "order-123" }
    );
    await expect(
      client.post(
        "/payment",
        { Amount: "200" },
        { idempotencyKey: "order-123" }
      )
    ).rejects.toThrow("different request parameters");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});

describe("DarajaClient environment boundaries", () => {
  it("rejects C2B simulation in production before network I/O", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "production", {
      idempotency: false,
    });

    await expect(
      simulateC2b(client, { phone: "254712345678", amount: 100 })
    ).rejects.toThrow("sandbox");
    expect(fetchMock).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});

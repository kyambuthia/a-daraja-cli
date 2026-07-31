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

  it("does not attach or cache idempotency for query POSTs", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockResolvedValueOnce(new Response(JSON.stringify({ value: 1 })));
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotency: false,
    });

    await expect(client.postWithoutIdempotency("/query", {})).resolves.toEqual({ value: 1 });
    const request = fetchMock.mock.calls[1]?.[1] as RequestInit;
    expect(request.headers).not.toHaveProperty("Idempotency-Key");
  });

  it("accepts an application-provided idempotency backend", async () => {
    const entries = new Map<string, { fingerprint: string; response: Buffer }>();
    const backend = {
      async acquire(key: string, fingerprint: string) {
        const entry = entries.get(key);
        if (!entry) return { kind: "acquired" as const };
        if (entry.fingerprint !== fingerprint) return { kind: "fingerprint-mismatch" as const };
        return { kind: "success" as const, response: entry.response };
      },
      async saveSuccess(key: string, response: Buffer) {
        entries.set(key, { fingerprint: "", response });
      },
      async saveFailed() {},
      async saveUnknown() {},
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockResolvedValueOnce(new Response(JSON.stringify({ ok: true })));
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotencyStore: backend,
    });

    await client.post("/payment", { Amount: "100" });
    expect(entries.size).toBe(1);
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

  it("redacts unrecognized fields from API errors", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            errorCode: "400.001",
            errorMessage: "Request rejected",
            consumerSecret: "must-not-leak",
          }),
          { status: 400 }
        )
      );
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotency: false,
    });

    const error = await client.post("/payment", {}).catch((value: unknown) => value);
    expect(String(error)).toContain("400.001");
    expect(String(error)).toContain("Request rejected");
    expect(String(error)).not.toContain("must-not-leak");
  });

  it("enforces the configured request timeout", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: "token", expires_in: "3599" }))
      )
      .mockImplementationOnce((_url: string, init: RequestInit) =>
        new Promise((_resolve, reject) => {
          init.signal?.addEventListener("abort", () => reject(init.signal?.reason));
        })
      );
    vi.stubGlobal("fetch", fetchMock);
    const client = new DarajaClient(credentials, "sandbox", {
      baseUrlOverride: "https://daraja.invalid",
      idempotency: false,
      requestTimeoutMs: 20,
    });

    await expect(client.post("/slow", {})).rejects.toBeDefined();
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

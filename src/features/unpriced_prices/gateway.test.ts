import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the Tauri invoke layer before importing the gateway.
// bindings.ts wraps every TAURI_INVOKE result in { status: "ok", data }
// or catches rejections and returns { status: "error", error }.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after the mock is registered so bindings.ts picks up the stub.
const { unpricedPricesGateway } = await import("./gateway");

describe("unpricedPricesGateway — recordPrice (MKT-175)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-175 — happy path: gateway forwards positional args to record_asset_price
  // and passes the ok Result through (F27: no throw).
  it("recordPrice passes through ok result and forwards positional args exactly", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await unpricedPricesGateway.recordPrice("asset-1", "2026-06-19", 12.5);

    expect(result).toEqual({ status: "ok", data: null });
    // bindings.ts calls TAURI_INVOKE("record_asset_price", { assetId, date, price })
    expect(mockInvoke).toHaveBeenCalledWith("record_asset_price", {
      assetId: "asset-1",
      date: "2026-06-19",
      price: 12.5,
    });
  });

  // F27 — error result is passed through, not thrown.
  it("recordPrice passes through NotPositive error result without throwing", async () => {
    mockInvoke.mockRejectedValue({ code: "NotPositive" });

    const result = await unpricedPricesGateway.recordPrice("asset-1", "2026-06-19", 0);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("NotPositive");
    }
    expect(mockInvoke).toHaveBeenCalledWith("record_asset_price", {
      assetId: "asset-1",
      date: "2026-06-19",
      price: 0,
    });
  });

  // F27 — DateInFuture error is passed through.
  it("recordPrice passes through DateInFuture error result without throwing", async () => {
    mockInvoke.mockRejectedValue({ code: "DateInFuture" });

    const result = await unpricedPricesGateway.recordPrice("asset-1", "2099-12-31", 50);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("DateInFuture");
    }
  });

  // F27 — AssetNotFound error is passed through.
  it("recordPrice passes through AssetNotFound error result without throwing", async () => {
    mockInvoke.mockRejectedValue({ code: "AssetNotFound", id: "asset-999" });

    const result = await unpricedPricesGateway.recordPrice("asset-999", "2026-06-19", 10);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("AssetNotFound");
    }
  });

  // F27 — DatabaseError is passed through.
  it("recordPrice passes through DatabaseError without throwing", async () => {
    mockInvoke.mockRejectedValue({ code: "DatabaseError" });

    const result = await unpricedPricesGateway.recordPrice("asset-1", "2026-06-19", 10);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("DatabaseError");
    }
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the gateways the launch helper reaches so no command.* call is made.
vi.mock("@/features/accounts/gateway", () => ({
  accountGateway: { fetchAllAssetPrices: vi.fn() },
}));
vi.mock("@/features/shell/gateway", () => ({
  shellGateway: { onMigrationError: vi.fn(() => Promise.resolve(() => {})) },
}));

import { accountGateway } from "@/features/accounts/gateway";
import { setAutoFetch } from "@/lib/autoFetchStorage";
// Import the testable launch helper extracted from App.tsx (MKT-121).
import { maybeLaunchAutoFetch } from "./App";

describe("maybeLaunchAutoFetch — MKT-121 launch dispatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear(); // auto-fetch off by default
  });

  // MKT-120 — auto-fetch disabled: no dispatch at all.
  it("does not dispatch when auto-fetch is disabled", async () => {
    setAutoFetch(false);
    await maybeLaunchAutoFetch();
    expect(accountGateway.fetchAllAssetPrices).not.toHaveBeenCalled();
  });

  // MKT-121 / ADR-017 — auto-fetch enabled: dispatch keylessly, no key gate.
  // PMV-010 / PMV-015 — the launch auto-fetch states its own trigger and must never
  // pass "Manual" — a launch fetch never reports movement.
  it("dispatches the keyless fetch with the Launch trigger when auto-fetch is enabled", async () => {
    setAutoFetch(true);
    vi.mocked(accountGateway.fetchAllAssetPrices).mockResolvedValue({ status: "ok", data: null });

    await maybeLaunchAutoFetch();

    expect(accountGateway.fetchAllAssetPrices).toHaveBeenCalledWith("Launch");
  });

  // A dispatch error is swallowed (logged), never thrown to the caller.
  it("does not throw when the dispatch returns an error", async () => {
    setAutoFetch(true);
    vi.mocked(accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "FetchAlreadyRunning" },
    });

    await expect(maybeLaunchAutoFetch()).resolves.toBeUndefined();
  });
});

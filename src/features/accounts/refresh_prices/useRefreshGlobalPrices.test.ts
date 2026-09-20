import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the accounts gateway before importing the hook under test.
// The hook calls accountGateway.fetchAllAssetPrices() — mock at this boundary
// so the hook never crosses into commands.* (F3).
vi.mock("../gateway", () => ({
  accountGateway: {
    fetchAllAssetPrices: vi.fn(),
  },
}));

// Mock the snackbar store — the hook dispatches snackbar messages on all branches.
const mockShowSnackbar = vi.hoisted(() => vi.fn());
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

import * as gateway from "../gateway";
import { useRefreshGlobalPrices } from "./useRefreshGlobalPrices";

describe("useRefreshGlobalPrices", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-133 — isPending starts false
  it("isPending is false before refresh is called", () => {
    const { result } = renderHook(() => useRefreshGlobalPrices());
    expect(result.current.isPending).toBe(false);
  });

  // MKT-115 / MKT-133 — success path: snackbar mkt.fetch_dispatched, isPending returns to false
  it("dispatches mkt.fetch_dispatched snackbar on successful fetch dispatch", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "ok",
      data: null,
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_dispatched", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-115 — FetchAlreadyRunning → snackbar mkt.fetch_already_running
  it("dispatches mkt.fetch_already_running snackbar on FetchAlreadyRunning error", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "FetchAlreadyRunning" },
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_already_running", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-115 — NoFetchableHoldings → snackbar mkt.fetch_no_holdings
  it("dispatches mkt.fetch_no_holdings snackbar on NoFetchableHoldings error", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "NoFetchableHoldings" },
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_no_holdings", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // DatabaseError → snackbar error.DatabaseError
  it("dispatches error.DatabaseError snackbar on DatabaseError", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // UnknownError → snackbar error.DatabaseError (generic fallback)
  it("dispatches error.DatabaseError snackbar on UnknownError", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "UnknownError" },
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-133 — isPending is true while fetch is in flight
  it("isPending is true while the fetch gateway call is in progress", async () => {
    let resolveFetch!: (v: { status: "ok"; data: null }) => void;
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockReturnValue(
      new Promise((resolve) => {
        resolveFetch = resolve;
      }),
    );

    const { result } = renderHook(() => useRefreshGlobalPrices());

    act(() => {
      void result.current.refresh();
    });

    await waitFor(() => expect(result.current.isPending).toBe(true));

    await act(async () => {
      resolveFetch({ status: "ok", data: null });
    });

    expect(result.current.isPending).toBe(false);
  });

  // ADR-017 — keyless: the fetch dispatches directly, exactly once, no key gate.
  // PMV-010 / PMV-015 — the Global refresh is the only path that reports movement,
  // so it must state its own trigger as "Manual".
  it("calls accountGateway.fetchAllAssetPrices exactly once per refresh call, with the Manual trigger", async () => {
    vi.mocked(gateway.accountGateway.fetchAllAssetPrices).mockResolvedValue({
      status: "ok",
      data: null,
    });

    const { result } = renderHook(() => useRefreshGlobalPrices());

    await act(async () => {
      await result.current.refresh();
    });

    expect(gateway.accountGateway.fetchAllAssetPrices).toHaveBeenCalledTimes(1);
    expect(gateway.accountGateway.fetchAllAssetPrices).toHaveBeenCalledWith("Manual");
  });
});

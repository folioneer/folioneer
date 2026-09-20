import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the account_details gateway before importing the hook under test.
// The hook calls accountDetailsGateway.fetchAccountAssetPrices(accountId) —
// mock at this boundary so the hook never crosses into commands.* (F3).
vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    fetchAccountAssetPrices: vi.fn(),
  },
}));

// Mock the snackbar store — the hook dispatches snackbar messages on all branches.
const mockShowSnackbar = vi.hoisted(() => vi.fn());
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

import * as gateway from "../gateway";
import { useRefreshAccountPrices } from "./useRefreshAccountPrices";

describe("useRefreshAccountPrices", () => {
  beforeEach(() => vi.clearAllMocks());

  // MKT-133 — isPending starts false
  it("isPending is false before refresh is called", () => {
    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-131 / MKT-132 — happy path: gateway called with the accountId only (keyless, ADR-017)
  it("calls fetchAccountAssetPrices with the given accountId on refresh", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "ok",
      data: null,
    });

    const accountId = "account-42";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(gateway.accountDetailsGateway.fetchAccountAssetPrices).toHaveBeenCalledWith(
      "account-42",
    );
  });

  // MKT-115 — success path: snackbar mkt.fetch_dispatched, isPending returns to false
  it("dispatches mkt.fetch_dispatched snackbar on successful fetch dispatch", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "ok",
      data: null,
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_dispatched", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-132 — AccountNotFound → snackbar error.AccountNotFound
  it("dispatches error.AccountNotFound snackbar on AccountNotFound error", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "AccountNotFound", account_id: "account-1" },
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.AccountNotFound", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-115 — FetchAlreadyRunning → snackbar mkt.fetch_already_running
  it("dispatches mkt.fetch_already_running snackbar on FetchAlreadyRunning error", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "FetchAlreadyRunning" },
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_already_running", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-115 — NoFetchableHoldings → snackbar mkt.fetch_no_holdings
  it("dispatches mkt.fetch_no_holdings snackbar on NoFetchableHoldings error", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "NoFetchableHoldings" },
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.fetch_no_holdings", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // DatabaseError → snackbar error.DatabaseError
  it("dispatches error.DatabaseError snackbar on DatabaseError", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // UnknownError → snackbar error.DatabaseError (generic fallback)
  it("dispatches error.DatabaseError snackbar on UnknownError", async () => {
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockResolvedValue({
      status: "error",
      error: { code: "UnknownError" },
    });

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", expect.any(String));
    expect(result.current.isPending).toBe(false);
  });

  // MKT-133 — isPending is true while fetch is in flight
  it("isPending is true while the fetch gateway call is in progress", async () => {
    let resolveFetch!: (v: { status: "ok"; data: null }) => void;
    vi.mocked(gateway.accountDetailsGateway.fetchAccountAssetPrices).mockReturnValue(
      new Promise((resolve) => {
        resolveFetch = resolve;
      }),
    );

    const accountId = "account-1";
    const { result } = renderHook(() => useRefreshAccountPrices(accountId));

    act(() => {
      void result.current.refresh();
    });

    await waitFor(() => expect(result.current.isPending).toBe(true));

    await act(async () => {
      resolveFetch({ status: "ok", data: null });
    });

    expect(result.current.isPending).toBe(false);
  });
});

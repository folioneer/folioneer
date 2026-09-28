import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useNonCashAssets } from "./useNonCashAssets";

const { mockGetNonCash, mockSubscribe, mockLogError, mockSnackbar } = vi.hoisted(() => ({
  mockGetNonCash: vi.fn(),
  mockSubscribe: vi.fn(),
  mockLogError: vi.fn(),
  mockSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  transactionGateway: {
    getNonCashAssets: () => mockGetNonCash(),
    subscribeToEvents: (cb: (type: string) => void) => mockSubscribe(cb),
  },
}));

vi.mock("@/lib/logger", () => ({ logger: { error: mockLogError } }));
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => mockSnackbar }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

const APPLE = { id: "aapl", name: "Apple" };
const BOND = { id: "bond", name: "Bond" };

describe("useNonCashAssets", () => {
  let eventCallback: ((type: string) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    mockSubscribe.mockImplementation(async (cb: (type: string) => void) => {
      eventCallback = cb;
      return () => {};
    });
  });

  // TRX-064 — the forms offer the core's list, reloaded when an asset changes
  it("loads the core's list and reloads it when assets change", async () => {
    mockGetNonCash.mockResolvedValue({ status: "ok", data: [APPLE] });
    const { result } = renderHook(() => useNonCashAssets());
    await act(async () => {});
    expect(result.current).toEqual([APPLE]);

    mockGetNonCash.mockResolvedValue({ status: "ok", data: [APPLE, BOND] });
    await act(async () => {
      eventCallback?.("TransactionUpdated");
    });
    expect(result.current).toEqual([APPLE]);

    await act(async () => {
      eventCallback?.("AssetUpdated");
    });
    expect(result.current).toEqual([APPLE, BOND]);
  });

  it("keeps an empty list and shows the generic error when the load fails", async () => {
    mockGetNonCash.mockResolvedValueOnce({ status: "error", error: { code: "DatabaseError" } });
    const { result } = renderHook(() => useNonCashAssets());
    await act(async () => {});
    expect(result.current).toEqual([]);
    expect(mockLogError).toHaveBeenCalledTimes(1);
    expect(mockSnackbar).toHaveBeenCalledWith("transaction.error_generic", "error");

    mockGetNonCash.mockRejectedValueOnce(new Error("down"));
    await act(async () => {
      eventCallback?.("SyncCompleted");
    });
    expect(mockLogError).toHaveBeenCalledTimes(2);
  });
});

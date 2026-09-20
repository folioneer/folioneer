import { beforeEach, describe, expect, it, vi } from "vitest";

const mockDebug = vi.hoisted(() => vi.fn());
const mockShow = vi.hoisted(() => vi.fn());

// Snackbar + i18n are used by the AssetPriceFetchCompleted handler (MKT-145).
vi.mock("../ui/components/snackbar/snackbarStore", () => ({
  useSnackbarStore: { getState: () => ({ show: mockShow }) },
}));
vi.mock("i18next", () => ({
  default: {
    t: (key: string, vars?: Record<string, unknown>) => `${key}:${JSON.stringify(vars ?? {})}`,
  },
}));

// Mock all gateways that store.ts imports so the module loads cleanly in tests
vi.mock("../features/accounts/gateway", () => ({
  accountGateway: { getAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }) },
}));

vi.mock("../features/assets/gateway", () => ({
  assetGateway: {
    getAssetsWithArchived: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

vi.mock("../features/categories/gateway", () => ({
  categoryGateway: { getCategories: vi.fn().mockResolvedValue({ status: "ok", data: [] }) },
}));

vi.mock("./logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), debug: mockDebug },
}));

// Capture the events.event.listen callback so tests can fire synthetic events
type UnpricedAssetFixture = {
  asset_id: string;
  name: string;
  reference: string;
  isin: string | null;
  currency: string;
  last_price: number | null;
  last_price_date: string | null;
};
type CapturedEvent = {
  payload: {
    type: string;
    ok?: number;
    skipped?: number;
    unpriced?: UnpricedAssetFixture[];
    done?: number;
    total?: number;
  };
};
let capturedEventListener: ((event: CapturedEvent) => void) | null = null;

vi.mock("../bindings", () => ({
  events: {
    event: {
      listen: vi.fn((cb: (event: CapturedEvent) => void) => {
        capturedEventListener = cb;
        return Promise.resolve(() => {});
      }),
    },
  },
}));

vi.mock("@tauri-apps/api/app", () => ({
  getName: vi.fn().mockResolvedValue("Folioneer"),
  getVersion: vi.fn().mockResolvedValue("0.0.0"),
}));

const { useAppStore } = await import("./store");

describe("store — locallyHandledEvents (FXR-037)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    // Reset store to un-initialized so init() runs fresh
    useAppStore.setState({ isInitialized: false });
  });

  // FXR-037 — CurrencyRateUpdated must be in locallyHandledEvents so it does NOT
  // trigger a global re-fetch AND does not emit an "unhandled event" debug log.
  // The test fires the event and asserts debug was NOT called (if it were truly
  // unhandled, logger.debug("[store] unhandled event", ...) would fire).
  it("FXR-037 — CurrencyRateUpdated is listed as locally handled (no debug log emitted)", async () => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    capturedEventListener?.({ payload: { type: "CurrencyRateUpdated" } });

    // If CurrencyRateUpdated is NOT in locallyHandledEvents, store.ts will call
    // logger.debug("[store] unhandled event", ...) — that is the failure signal.
    expect(mockDebug).not.toHaveBeenCalledWith(
      "[store] unhandled event",
      expect.objectContaining({ type: "CurrencyRateUpdated" }),
    );

    cleanup();
  });

  // Tighter assertion: CurrencyRateUpdated must not trigger any global gateway call
  it("FXR-037 — CurrencyRateUpdated does not trigger fetchAssets / fetchAccounts / fetchCategories", async () => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    const { assetGateway } = await import("../features/assets/gateway");
    const { accountGateway } = await import("../features/accounts/gateway");
    const { categoryGateway } = await import("../features/categories/gateway");

    const assetCallsBefore = vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length;
    const accountCallsBefore = vi.mocked(accountGateway.getAccounts).mock.calls.length;
    const categoryCallsBefore = vi.mocked(categoryGateway.getCategories).mock.calls.length;

    capturedEventListener?.({ payload: { type: "CurrencyRateUpdated" } });

    expect(vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length).toBe(assetCallsBefore);
    expect(vi.mocked(accountGateway.getAccounts).mock.calls.length).toBe(accountCallsBefore);
    expect(vi.mocked(categoryGateway.getCategories).mock.calls.length).toBe(categoryCallsBefore);

    cleanup();
  });

  // HNO-043 / FXR-056 — the two events raised by note and pair writes are handled by their
  // own views; the global store must neither re-fetch nor log them as unhandled.
  it.each([
    "HoldingNoteUpdated",
    "CurrencyPairUpdated",
  ])("%s is listed as locally handled (no debug log emitted)", async (type) => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    capturedEventListener?.({ payload: { type } });

    expect(mockDebug).not.toHaveBeenCalledWith(
      "[store] unhandled event",
      expect.objectContaining({ type }),
    );

    cleanup();
  });

  // Sanity: AssetUpdated still triggers fetchAssets (regression guard)
  it("AssetUpdated still triggers fetchAssets (regression guard for FXR-037 scope)", async () => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    const { assetGateway } = await import("../features/assets/gateway");
    const callsBefore = vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length;

    capturedEventListener?.({ payload: { type: "AssetUpdated" } });

    expect(vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length).toBeGreaterThan(
      callsBefore,
    );

    cleanup();
  });
});

describe("store — AssetPriceFetchCompleted snackbar (MKT-145)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ isInitialized: false });
  });

  it("stays silent when nothing was skipped (success)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 5, skipped: 0, unpriced: [] },
    });

    expect(mockShow).not.toHaveBeenCalled();
    cleanup();
  });

  it("shows an error snackbar when every asset was skipped (ok == 0)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 0, skipped: 3, unpriced: [] },
    });

    expect(mockShow).toHaveBeenCalledTimes(1);
    expect(mockShow).toHaveBeenCalledWith(
      expect.stringMatching(/mkt\.fetch_completed_failed.*"skipped":3/),
      "error",
    );
    cleanup();
  });

  it("shows an info snackbar on partial success (ok > 0 and skipped > 0)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 2, skipped: 1, unpriced: [] },
    });

    expect(mockShow).toHaveBeenCalledTimes(1);
    expect(mockShow).toHaveBeenCalledWith(
      expect.stringMatching(/mkt\.fetch_completed_partial.*"ok":2.*"skipped":1/),
      "info",
    );
    cleanup();
  });
});

// ---------------------------------------------------------------------------
// MKT-172 / MKT-173 — unpricedAssets store slice and snackbar suppression
// ---------------------------------------------------------------------------

const makeUnpricedAsset = (id: string): UnpricedAssetFixture => ({
  asset_id: id,
  name: `Asset ${id}`,
  reference: `REF-${id}`,
  isin: null,
  currency: "EUR",
  last_price: null,
  last_price_date: null,
});

describe("store — unpricedAssets slice (MKT-172)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ isInitialized: false });
  });

  // MKT-172 — when payload.unpriced is non-empty, the store stashes it in the
  // unpricedAssets slice so UnpricedPricesModalMount can open the modal.
  it("stashes the unpriced list into the unpricedAssets store slice when non-empty (MKT-172)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    const unpriced = [makeUnpricedAsset("asset-1"), makeUnpricedAsset("asset-2")];
    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 0, skipped: 2, unpriced },
    });

    const state = useAppStore.getState();
    // The implementation must expose unpricedAssets on the store state.
    expect(state.unpricedAssets).toEqual(unpriced);

    cleanup();
  });

  // MKT-172 — when unpriced is empty, the slice remains empty (or is cleared).
  it("does not stash anything when unpriced list is empty (no modal needed)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 5, skipped: 0, unpriced: [] },
    });

    const unpricedAssets = useAppStore.getState().unpricedAssets;
    // Slice must be empty.
    expect(unpricedAssets).toHaveLength(0);

    cleanup();
  });
});

describe("store — MKT-145 snackbar suppression when unpriced non-empty (MKT-173)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ isInitialized: false });
  });

  // MKT-173 — when unpriced is non-empty, the modal supersedes the snackbar:
  // the MKT-145 snackbar must NOT be shown even though skipped > 0.
  it("suppresses the MKT-145 snackbar when unpriced list is non-empty (MKT-173)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    const unpriced = [makeUnpricedAsset("asset-1")];
    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 0, skipped: 1, unpriced },
    });

    // The modal supersedes the snackbar — no snackbar call allowed.
    expect(mockShow).not.toHaveBeenCalled();

    cleanup();
  });

  it("suppresses the snackbar on partial success when unpriced is non-empty (MKT-173)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    const unpriced = [makeUnpricedAsset("asset-1")];
    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 3, skipped: 1, unpriced },
    });

    expect(mockShow).not.toHaveBeenCalled();

    cleanup();
  });

  // When unpriced is empty, the snackbar still fires as before (regression guard).
  it("still shows the snackbar when unpriced is empty and skipped > 0 (MKT-145 regression guard)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 0, skipped: 2, unpriced: [] },
    });

    expect(mockShow).toHaveBeenCalledTimes(1);

    cleanup();
  });
});

describe("store — unpricedAssets dismiss / clear action (MKT-177)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ isInitialized: false });
  });

  // MKT-177 — after the modal is dismissed (all rows resolved or skipped),
  // a dismiss action empties the unpricedAssets slice.
  it("clearUnpricedAssets action empties the slice (MKT-177)", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    // First stash some assets.
    capturedEventListener?.({
      payload: {
        type: "AssetPriceFetchCompleted",
        ok: 0,
        skipped: 1,
        unpriced: [makeUnpricedAsset("asset-1")],
      },
    });

    // Verify they are stashed.
    expect(useAppStore.getState().unpricedAssets).toHaveLength(1);

    // Call the dismiss/clear action.
    useAppStore.getState().clearUnpricedAssets();

    // Slice must now be empty.
    expect(useAppStore.getState().unpricedAssets).toHaveLength(0);

    cleanup();
  });
});

// ---------------------------------------------------------------------------
// SyncCompleted — SYN-064/D10: a bare marker event; the frontend treats it as
// a global refresh (accounts, assets, categories — the store's cached slices)
// rather than reading a payload, since the run's outcome is re-read via
// get_sync_status separately (SyncSection/SyncIndicator own that read).
// ---------------------------------------------------------------------------

describe("store — SyncCompleted global refresh (SYN-064)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({ isInitialized: false });
  });

  it("triggers a global refresh of assets, categories and accounts on SyncCompleted", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    const { assetGateway } = await import("../features/assets/gateway");
    const { accountGateway } = await import("../features/accounts/gateway");
    const { categoryGateway } = await import("../features/categories/gateway");

    const assetCallsBefore = vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length;
    const accountCallsBefore = vi.mocked(accountGateway.getAccounts).mock.calls.length;
    const categoryCallsBefore = vi.mocked(categoryGateway.getCategories).mock.calls.length;

    capturedEventListener?.({ payload: { type: "SyncCompleted" } });

    expect(vi.mocked(assetGateway.getAssetsWithArchived).mock.calls.length).toBeGreaterThan(
      assetCallsBefore,
    );
    expect(vi.mocked(accountGateway.getAccounts).mock.calls.length).toBeGreaterThan(
      accountCallsBefore,
    );
    expect(vi.mocked(categoryGateway.getCategories).mock.calls.length).toBeGreaterThan(
      categoryCallsBefore,
    );

    cleanup();
  });

  // Regression guard for the eventMap fallback: today SyncCompleted is not
  // wired, so it falls into "unhandled event" — this must stop being true.
  it("does not log an unhandled-event warning for SyncCompleted", async () => {
    const cleanup = useAppStore.getState().init();
    await vi.waitFor(() => expect(capturedEventListener).not.toBeNull());

    capturedEventListener?.({ payload: { type: "SyncCompleted" } });

    expect(mockDebug).not.toHaveBeenCalledWith(
      "[store] unhandled event",
      expect.objectContaining({ type: "SyncCompleted" }),
    );

    cleanup();
  });
});

describe("store — price-fetch progress (MKT-180)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedEventListener = null;
    useAppStore.setState({
      isInitialized: false,
      priceFetch: { active: false, done: 0, total: 0 },
    });
  });

  it("tracks AssetPriceFetchProgress into the priceFetch slice (MKT-180)", async () => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    capturedEventListener?.({ payload: { type: "AssetPriceFetchProgress", done: 0, total: 5 } });
    expect(useAppStore.getState().priceFetch).toEqual({ active: true, done: 0, total: 5 });

    capturedEventListener?.({ payload: { type: "AssetPriceFetchProgress", done: 3, total: 5 } });
    expect(useAppStore.getState().priceFetch).toEqual({ active: true, done: 3, total: 5 });

    cleanup();
  });

  it("clears the slice on AssetPriceFetchCompleted (MKT-180)", async () => {
    const cleanup = useAppStore.getState().init();
    await new Promise((r) => setTimeout(r, 0));

    capturedEventListener?.({ payload: { type: "AssetPriceFetchProgress", done: 2, total: 2 } });
    capturedEventListener?.({
      payload: { type: "AssetPriceFetchCompleted", ok: 2, skipped: 0, unpriced: [] },
    });
    expect(useAppStore.getState().priceFetch).toEqual({ active: false, done: 0, total: 0 });

    cleanup();
  });
});

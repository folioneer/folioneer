import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Exchange } from "@/bindings";

// Mock the gateway before importing the store so the store's lazy-load action
// calls the mock rather than the real gateway.
const mockGetSupportedExchanges = vi.fn();

vi.mock("./gateway", () => ({
  assetGateway: {
    getSupportedExchanges: mockGetSupportedExchanges,
  },
}));

// Dynamic import after mocks are registered so the store's module-level
// references to the gateway pick up the mock.
const { useAssetsStore } = await import("./store");

const SAMPLE_EXCHANGES: Exchange[] = [
  { code: "XPAR", label: "Euronext Paris" },
  { code: "XNAS", label: "NASDAQ" },
];

describe("useAssetsStore — supportedExchanges slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset Zustand state to initial between tests
    useAssetsStore.setState({ supportedExchanges: [] });
  });

  // (a) initial state
  it("starts with an empty supportedExchanges list", () => {
    const state = useAssetsStore.getState();
    expect(state.supportedExchanges).toEqual([]);
  });

  // (b) loadSupportedExchanges populates state from the gateway (session-static)
  it("loadSupportedExchanges fetches from gateway and populates the store when empty", async () => {
    mockGetSupportedExchanges.mockResolvedValue(SAMPLE_EXCHANGES);

    await useAssetsStore.getState().loadSupportedExchanges();

    expect(mockGetSupportedExchanges).toHaveBeenCalledTimes(1);
    expect(useAssetsStore.getState().supportedExchanges).toEqual(SAMPLE_EXCHANGES);
  });

  // (c) loadSupportedExchanges is a no-op when state is already populated
  it("loadSupportedExchanges is a no-op when store already has exchanges (session-static cache)", async () => {
    // Pre-seed the store
    useAssetsStore.setState({ supportedExchanges: SAMPLE_EXCHANGES });

    await useAssetsStore.getState().loadSupportedExchanges();

    expect(mockGetSupportedExchanges).not.toHaveBeenCalled();
    expect(useAssetsStore.getState().supportedExchanges).toEqual(SAMPLE_EXCHANGES);
  });
});

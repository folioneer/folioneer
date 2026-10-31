import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { usePriceProvider } from "./usePriceProvider";

const { getPriceProviderChoice, setPriceProviderChoice, fetchAllAssetPrices } = vi.hoisted(() => ({
  getPriceProviderChoice: vi.fn(),
  setPriceProviderChoice: vi.fn(),
  fetchAllAssetPrices: vi.fn(),
}));

vi.mock("../gateway", () => ({
  getPriceProviderChoice,
  setPriceProviderChoice,
  fetchAllAssetPrices,
}));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn(), warn: vi.fn() } }));

const YAHOO = "yahoo_finance";
const off = { chosen: null, offered: [YAHOO] };
const on = { chosen: YAHOO, offered: [YAHOO] };
const refreshExternalProvider = vi.fn();

const opened = async () => {
  const hook = renderHook(() => usePriceProvider());
  await waitFor(() => expect(hook.result.current.isLoading).toBe(false));
  return hook;
};

describe("usePriceProvider (MKT-215)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getPriceProviderChoice.mockResolvedValue(off);
    setPriceProviderChoice.mockResolvedValue({ status: "ok", data: on });
    refreshExternalProvider.mockResolvedValue(undefined);
    fetchAllAssetPrices.mockResolvedValue({ status: "ok", data: null });
    useAppStore.setState({ refreshExternalProvider });
  });

  it("reads Off, by default, with what the build offers", async () => {
    const { result } = await opened();

    expect(result.current.selected).toBe("off");
    expect(result.current.choice?.offered).toEqual([YAHOO]);
  });

  it("saves nothing on choosing a provider: it shows the notice first", async () => {
    const { result } = await opened();

    act(() => result.current.select(YAHOO));

    expect(result.current.selected).toBe(YAHOO);
    expect(result.current.awaitingAcknowledgement).toBe(YAHOO);
    expect(setPriceProviderChoice).not.toHaveBeenCalled();
  });

  it("makes it the choice once acknowledged, asks again what it was told, and fetches", async () => {
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(setPriceProviderChoice).toHaveBeenCalledWith(YAHOO);
    expect(result.current.choice?.chosen).toBe(YAHOO);
    expect(result.current.awaitingAcknowledgement).toBeNull();
    expect(refreshExternalProvider).toHaveBeenCalledTimes(1);
    expect(fetchAllAssetPrices).toHaveBeenCalledTimes(1);
  });

  it("is pending while the choice saves", async () => {
    let answer: (value: unknown) => void = () => {};
    setPriceProviderChoice.mockReturnValue(
      new Promise((resolve) => {
        answer = resolve;
      }),
    );
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    act(() => result.current.acknowledge());
    expect(result.current.isSaving).toBe(true);

    await act(async () => answer({ status: "ok", data: on }));
    expect(result.current.isSaving).toBe(false);
  });

  it("shows a refusal and keeps the choice as it was: no re-read, no fetch", async () => {
    setPriceProviderChoice.mockResolvedValue({ status: "error", error: { code: "NotSaved" } });
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(result.current.error).toEqual({ key: "error.price_provider.NotSaved" });
    expect(result.current.choice?.chosen).toBeNull();
    expect(refreshExternalProvider).not.toHaveBeenCalled();
    expect(fetchAllAssetPrices).not.toHaveBeenCalled();
  });

  it("keeps the notice after a refusal, to try again", async () => {
    setPriceProviderChoice.mockResolvedValue({ status: "error", error: { code: "NotSaved" } });
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(result.current.awaitingAcknowledgement).toBe(YAHOO);
  });

  it("shows a choice that threw as not saved", async () => {
    setPriceProviderChoice.mockRejectedValue(new Error("gone"));
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(result.current.error).toEqual({ key: "error.price_provider.NotSaved" });
    expect(result.current.isSaving).toBe(false);
  });

  it("keeps the provider when turning it off is refused", async () => {
    getPriceProviderChoice.mockResolvedValue(on);
    setPriceProviderChoice.mockResolvedValue({ status: "error", error: { code: "NotSaved" } });
    const { result } = await opened();

    await act(async () => result.current.select("off"));

    expect(result.current.selected).toBe(YAHOO);
    expect(result.current.error).toEqual({ key: "error.price_provider.NotSaved" });
  });

  it("keeps the choice when the first fetch is refused or throws", async () => {
    fetchAllAssetPrices.mockResolvedValueOnce({ status: "error", error: { code: "x" } });
    const { result } = await opened();
    act(() => result.current.select(YAHOO));
    await act(async () => result.current.acknowledge());
    expect(result.current.choice?.chosen).toBe(YAHOO);

    fetchAllAssetPrices.mockRejectedValueOnce(new Error("gone"));
    getPriceProviderChoice.mockResolvedValue(off);
    const second = await opened();
    act(() => second.result.current.select(YAHOO));
    await act(async () => second.result.current.acknowledge());
    expect(second.result.current.choice?.chosen).toBe(YAHOO);
    expect(second.result.current.error).toBeNull();
  });

  it("turns off at once, with no notice and no fetch", async () => {
    getPriceProviderChoice.mockResolvedValue(on);
    setPriceProviderChoice.mockResolvedValue({ status: "ok", data: off });
    const { result } = await opened();

    await act(async () => result.current.select("off"));

    expect(setPriceProviderChoice).toHaveBeenCalledWith(null);
    expect(result.current.selected).toBe("off");
    expect(refreshExternalProvider).toHaveBeenCalledTimes(1);
    expect(fetchAllAssetPrices).not.toHaveBeenCalled();
  });

  it("going back to Off before acknowledging saves nothing", async () => {
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    act(() => result.current.select("off"));

    expect(result.current.awaitingAcknowledgement).toBeNull();
    expect(setPriceProviderChoice).not.toHaveBeenCalled();
  });

  it("says when the source cannot be read", async () => {
    getPriceProviderChoice.mockRejectedValue(new Error("gone"));
    const { result } = await opened();

    expect(result.current.choice).toBeNull();
    expect(result.current.error).toEqual({ key: "settings.price_provider_load_failed" });
  });
});

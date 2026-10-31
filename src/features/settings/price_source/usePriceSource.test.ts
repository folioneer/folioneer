import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { usePriceSource } from "./usePriceSource";

const { getPriceSource, setPriceSource, fetchPricesOnChoosingSource } = vi.hoisted(() => ({
  getPriceSource: vi.fn(),
  setPriceSource: vi.fn(),
  fetchPricesOnChoosingSource: vi.fn(),
}));

vi.mock("../gateway", () => ({ getPriceSource, setPriceSource, fetchPricesOnChoosingSource }));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn(), warn: vi.fn() } }));

const YAHOO = "yahoo_finance";
const off = { chosen: null, offered: [YAHOO] };
const on = { chosen: YAHOO, offered: [YAHOO] };
const refreshExternalProvider = vi.fn();

const opened = async () => {
  const hook = renderHook(() => usePriceSource());
  await waitFor(() => expect(hook.result.current.isLoading).toBe(false));
  return hook;
};

describe("usePriceSource (MKT-215)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getPriceSource.mockResolvedValue(off);
    setPriceSource.mockResolvedValue({ status: "ok", data: on });
    refreshExternalProvider.mockResolvedValue(undefined);
    fetchPricesOnChoosingSource.mockResolvedValue(undefined);
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
    expect(setPriceSource).not.toHaveBeenCalled();
  });

  it("makes it the choice once acknowledged, asks again what it was told, and fetches", async () => {
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(setPriceSource).toHaveBeenCalledWith(YAHOO);
    expect(result.current.choice?.chosen).toBe(YAHOO);
    expect(result.current.awaitingAcknowledgement).toBeNull();
    expect(refreshExternalProvider).toHaveBeenCalledTimes(1);
    expect(fetchPricesOnChoosingSource).toHaveBeenCalledTimes(1);
  });

  it("is pending while the choice saves", async () => {
    let answer: (value: unknown) => void = () => {};
    setPriceSource.mockReturnValue(
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
    setPriceSource.mockResolvedValue({ status: "error", error: { code: "NotSaved" } });
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    await act(async () => result.current.acknowledge());

    expect(result.current.error).toEqual({ key: "error.price_provider.NotSaved" });
    expect(result.current.choice?.chosen).toBeNull();
    expect(refreshExternalProvider).not.toHaveBeenCalled();
    expect(fetchPricesOnChoosingSource).not.toHaveBeenCalled();
  });

  it("turns off at once, with no notice and no fetch", async () => {
    getPriceSource.mockResolvedValue(on);
    setPriceSource.mockResolvedValue({ status: "ok", data: off });
    const { result } = await opened();

    await act(async () => result.current.select("off"));

    expect(setPriceSource).toHaveBeenCalledWith(null);
    expect(result.current.selected).toBe("off");
    expect(refreshExternalProvider).toHaveBeenCalledTimes(1);
    expect(fetchPricesOnChoosingSource).not.toHaveBeenCalled();
  });

  it("going back to Off before acknowledging saves nothing", async () => {
    const { result } = await opened();
    act(() => result.current.select(YAHOO));

    act(() => result.current.select("off"));

    expect(result.current.awaitingAcknowledgement).toBeNull();
    expect(setPriceSource).not.toHaveBeenCalled();
  });

  it("says when the source cannot be read", async () => {
    getPriceSource.mockRejectedValue(new Error("gone"));
    const { result } = await opened();

    expect(result.current.choice).toBeNull();
    expect(result.current.error).toEqual({ key: "settings.price_source_load_failed" });
  });
});

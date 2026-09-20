import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useSettings } from "./useSettings";

const AUTO_FETCH_KEY = "auto_fetch_prices";

const AUTO_RECORD_PRICE_KEY = "auto_record_price";

describe("useSettings", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  // MKT-050 — autoRecordPrice defaults to false when localStorage key is absent
  it("autoRecordPrice defaults to false when localStorage key is absent", () => {
    const { result } = renderHook(() => useSettings());
    expect(result.current.autoRecordPrice).toBe(false);
  });

  // MKT-050 — autoRecordPrice is true when localStorage key is "true"
  it("autoRecordPrice is true when localStorage key is set to true", () => {
    localStorage.setItem(AUTO_RECORD_PRICE_KEY, "true");
    const { result } = renderHook(() => useSettings());
    expect(result.current.autoRecordPrice).toBe(true);
  });

  // MKT-050 — toggleAutoRecordPrice toggles state and persists to localStorage
  it("toggleAutoRecordPrice flips from false to true and persists to localStorage", () => {
    const { result } = renderHook(() => useSettings());

    expect(result.current.autoRecordPrice).toBe(false);

    act(() => {
      result.current.toggleAutoRecordPrice();
    });

    expect(result.current.autoRecordPrice).toBe(true);
    expect(localStorage.getItem(AUTO_RECORD_PRICE_KEY)).toBe("true");
  });

  // MKT-050 — toggleAutoRecordPrice toggles from true back to false and persists
  it("toggleAutoRecordPrice flips from true back to false and persists to localStorage", () => {
    localStorage.setItem(AUTO_RECORD_PRICE_KEY, "true");
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.toggleAutoRecordPrice();
    });

    expect(result.current.autoRecordPrice).toBe(false);
    expect(localStorage.getItem(AUTO_RECORD_PRICE_KEY)).toBe("false");
  });

  // MKT-120 — autoFetch defaults to false when localStorage key is absent
  it("autoFetch defaults to false when localStorage key is absent", () => {
    const { result } = renderHook(() => useSettings());
    expect(result.current.autoFetch).toBe(false);
  });

  // MKT-120 — autoFetch is true when localStorage key is "true"
  it("autoFetch is true when localStorage key is set to true", () => {
    localStorage.setItem(AUTO_FETCH_KEY, "true");
    const { result } = renderHook(() => useSettings());
    expect(result.current.autoFetch).toBe(true);
  });

  // MKT-120 — toggleAutoFetch flips from false to true and persists to localStorage
  it("toggleAutoFetch flips from false to true and persists to localStorage", () => {
    const { result } = renderHook(() => useSettings());

    expect(result.current.autoFetch).toBe(false);

    act(() => {
      result.current.toggleAutoFetch();
    });

    expect(result.current.autoFetch).toBe(true);
    expect(localStorage.getItem(AUTO_FETCH_KEY)).toBe("true");
  });

  // MKT-120 — toggleAutoFetch flips from true back to false and persists
  it("toggleAutoFetch flips from true back to false and persists to localStorage", () => {
    localStorage.setItem(AUTO_FETCH_KEY, "true");
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.toggleAutoFetch();
    });

    expect(result.current.autoFetch).toBe(false);
    expect(localStorage.getItem(AUTO_FETCH_KEY)).toBe("false");
  });
});

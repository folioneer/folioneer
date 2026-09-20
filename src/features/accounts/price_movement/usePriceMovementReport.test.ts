import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Event, PriceMovementReport } from "@/bindings";

// PMV-016 — mock the gateway boundary (F3); the hook must never reach
// events.* / commands.* directly.
vi.mock("../gateway", () => ({
  accountGateway: {
    subscribeToPriceFetchCompleted: vi.fn(),
  },
}));

import * as gateway from "../gateway";
import { usePriceMovementReport } from "./usePriceMovementReport";

type CompletedPayload = Extract<Event, { type: "AssetPriceFetchCompleted" }>;

const makeReport = (overrides: Partial<PriceMovementReport> = {}): PriceMovementReport => ({
  rows: [],
  total_before: 1_000_000,
  total_after: 1_100_000,
  total_currency: "EUR",
  total_movement_pct: 10_000_000,
  total_movement_amount: 100_000,
  observed_from: "2026-09-09",
  observed_to: "2026-09-11",
  incomplete: false,
  ...overrides,
});

const makePayload = (movement: PriceMovementReport | null): CompletedPayload => ({
  type: "AssetPriceFetchCompleted",
  ok: 1,
  skipped: 0,
  unpriced: [],
  movement,
});

describe("usePriceMovementReport", () => {
  beforeEach(() => vi.clearAllMocks());

  it("starts with no report", () => {
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockResolvedValue(vi.fn());

    const { result } = renderHook(() => usePriceMovementReport());

    expect(result.current.report).toBeNull();
  });

  it("subscribes to the gateway on mount", async () => {
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockResolvedValue(vi.fn());

    renderHook(() => usePriceMovementReport());

    await waitFor(() =>
      expect(gateway.accountGateway.subscribeToPriceFetchCompleted).toHaveBeenCalledTimes(1),
    );
  });

  it("stores the movement report when a completed payload carries one", async () => {
    let captured!: (payload: CompletedPayload) => void;
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockImplementation(
      async (cb) => {
        captured = cb;
        return vi.fn() as unknown as () => void;
      },
    );
    const report = makeReport();

    const { result } = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(captured).toBeDefined());

    act(() => captured(makePayload(report)));

    expect(result.current.report).toEqual(report);
  });

  // PMV-010/014 — the Launch auto-fetch (and any report that couldn't be
  // produced) publishes `movement: null`; the dialog must never appear for it.
  it("ignores a completed payload whose movement is null", async () => {
    let captured!: (payload: CompletedPayload) => void;
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockImplementation(
      async (cb) => {
        captured = cb;
        return vi.fn() as unknown as () => void;
      },
    );

    const { result } = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(captured).toBeDefined());

    act(() => captured(makePayload(null)));

    expect(result.current.report).toBeNull();
  });

  // PMV-061 — dismiss clears the report; nothing brings it back short of a
  // fresh AssetPriceFetchCompleted event.
  it("dismiss clears the stored report", async () => {
    let captured!: (payload: CompletedPayload) => void;
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockImplementation(
      async (cb) => {
        captured = cb;
        return vi.fn() as unknown as () => void;
      },
    );
    const report = makeReport();

    const { result } = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(captured).toBeDefined());
    act(() => captured(makePayload(report)));
    expect(result.current.report).toEqual(report);

    act(() => result.current.dismiss());

    expect(result.current.report).toBeNull();
  });

  // PMV-061 — dismissing does not resubscribe or otherwise resurrect the report.
  it("dismiss does not bring the report back on its own", async () => {
    let captured!: (payload: CompletedPayload) => void;
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockImplementation(
      async (cb) => {
        captured = cb;
        return vi.fn() as unknown as () => void;
      },
    );
    const { result } = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(captured).toBeDefined());
    act(() => captured(makePayload(makeReport())));
    act(() => result.current.dismiss());

    expect(result.current.report).toBeNull();
    expect(gateway.accountGateway.subscribeToPriceFetchCompleted).toHaveBeenCalledTimes(1);
  });

  // PMV-016 — the report belongs to the mounted surface: unmounting removes
  // the listener, so nothing is captured for a later return.
  it("unsubscribes from the gateway on unmount", async () => {
    const unlisten = vi.fn();
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockResolvedValue(unlisten);

    const { unmount } = renderHook(() => usePriceMovementReport());
    await waitFor(() =>
      expect(gateway.accountGateway.subscribeToPriceFetchCompleted).toHaveBeenCalledTimes(1),
    );

    unmount();

    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  // PMV-016 — the report belongs to the mounted surface. With a fake that honours
  // `dispose()`, a report published between an unmount and the next mount reaches
  // nobody, and the new mount starts empty — yet still receives what is published
  // after it. The positive half (the fresh report lands) proves the fake delivers.
  it("does not carry a report published while unmounted into a later mount", async () => {
    const listeners = new Set<(payload: CompletedPayload) => void>();
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockImplementation(
      async (cb) => {
        listeners.add(cb);
        return () => {
          listeners.delete(cb);
        };
      },
    );
    const publish = (report: PriceMovementReport) => {
      for (const listener of [...listeners]) listener(makePayload(report));
    };

    const first = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(listeners.size).toBe(1));
    first.unmount();
    await waitFor(() => expect(listeners.size).toBe(0));

    act(() => publish(makeReport({ total_after: 1_200_000 })));

    const second = renderHook(() => usePriceMovementReport());
    await waitFor(() => expect(listeners.size).toBe(1));
    expect(second.result.current.report).toBeNull();

    const fresh = makeReport({ total_after: 1_300_000 });
    act(() => publish(fresh));
    expect(second.result.current.report).toEqual(fresh);
  });

  // PMV-016 — the race the `cancelled` flag exists for: the component unmounts
  // BEFORE the subscribe promise resolves. Without the flag the listener arrives
  // after cleanup has run and is never disposed, so a report could still land on
  // a surface the user has left. The other unmount test awaits the call first,
  // so it never reaches this branch.
  it("disposes a subscription that resolves after unmount", async () => {
    const unlisten = vi.fn();
    let resolveSubscribe!: (dispose: () => void) => void;
    vi.mocked(gateway.accountGateway.subscribeToPriceFetchCompleted).mockReturnValue(
      new Promise<() => void>((resolve) => {
        resolveSubscribe = resolve;
      }),
    );

    const { unmount } = renderHook(() => usePriceMovementReport());
    unmount();
    expect(unlisten).not.toHaveBeenCalled();

    await act(async () => {
      resolveSubscribe(unlisten);
    });

    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

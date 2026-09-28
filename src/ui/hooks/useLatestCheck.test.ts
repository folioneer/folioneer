import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { type CheckResult, useLatestCheck } from "./useLatestCheck";

type Answer = CheckResult<number, string>;

describe("useLatestCheck", () => {
  it("returns the data of an ok answer, nothing before it arrives", async () => {
    const check = vi.fn(async (n: number): Promise<Answer> => ({ status: "ok", data: n * 2 }));
    const { result } = renderHook(() => useLatestCheck(21, check));

    expect(result.current).toEqual({ data: null, error: null, failed: false });
    await waitFor(() => expect(result.current.data).toBe(42));
    expect(check).toHaveBeenCalledWith(21);
  });

  it("returns the error of an error answer", async () => {
    const check = async (): Promise<Answer> => ({ status: "error", error: "too big" });
    const { result } = renderHook(() => useLatestCheck(1, check));

    await waitFor(() => expect(result.current.error).toBe("too big"));
    expect(result.current.data).toBeNull();
  });

  it("drops an earlier answer that arrives after a later one", async () => {
    let answerFirst: (value: Answer) => void = () => {};
    const check = vi
      .fn<(n: number) => Promise<Answer>>()
      .mockImplementationOnce(() => new Promise((resolve) => (answerFirst = resolve)))
      .mockResolvedValueOnce({ status: "error", error: "later" });
    const { result, rerender } = renderHook(({ n }) => useLatestCheck(n, check), {
      initialProps: { n: 1 },
    });

    rerender({ n: 2 });
    await waitFor(() => expect(result.current.error).toBe("later"));
    await act(async () => {
      answerFirst({ status: "ok", data: 1 });
    });

    expect(result.current).toEqual({ data: null, error: "later", failed: false });
  });

  it("reports a check that throws as failed and hands the cause over", async () => {
    const cause = new Error("down");
    const onFailure = vi.fn();
    const check = async (): Promise<Answer> => {
      throw cause;
    };
    const { result } = renderHook(() => useLatestCheck(1, check, onFailure));

    await waitFor(() => expect(result.current.failed).toBe(true));
    expect(onFailure).toHaveBeenCalledWith(cause);
  });

  it("runs nothing for a null request, and again only when the request changes by value", async () => {
    const check = vi.fn(async (r: { n: number }): Promise<Answer> => ({ status: "ok", data: r.n }));
    const { result, rerender } = renderHook(({ r }) => useLatestCheck(r, check), {
      initialProps: { r: null as { n: number } | null },
    });
    await act(async () => {});
    expect(check).not.toHaveBeenCalled();

    rerender({ r: { n: 1 } });
    await waitFor(() => expect(result.current.data).toBe(1));
    rerender({ r: { n: 1 } });
    await act(async () => {});
    expect(check).toHaveBeenCalledTimes(1);
  });
});

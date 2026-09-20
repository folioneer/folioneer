import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConflictNotice } from "@/bindings";

// 1. Mock the gateway module before importing the hook (test_convention.md § Mocking gateway modules)
vi.mock("../../gateway", () => ({
  dismissConflictNotice: vi.fn(),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../../gateway";
import { useNoticeList } from "./useNoticeList";

function makeNotice(overrides: Partial<ConflictNotice> = {}): ConflictNotice {
  return {
    notice_id: "notice-1",
    kind: "OverruledEdit",
    record_kind: "Transaction",
    record_identity: "tx-1",
    record_label: "Sell 10 AAPL on 2026-08-01",
    other_device_id: "device-2",
    other_device_name: "Laptop",
    raised_at: "2026-08-20T10:00:00Z",
    ...overrides,
  };
}

describe("useNoticeList — dismiss (SYN-066)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("removes the notice from the list after a successful dismiss", async () => {
    vi.mocked(gateway.dismissConflictNotice).mockResolvedValue({ status: "ok", data: null });
    const notices = [makeNotice({ notice_id: "notice-1" }), makeNotice({ notice_id: "notice-2" })];
    const onDismissed = vi.fn();

    const { result } = renderHook(() => useNoticeList({ notices, onDismissed }));

    await act(async () => {
      await result.current.handleDismiss("notice-1");
    });

    expect(gateway.dismissConflictNotice).toHaveBeenCalledWith("notice-1");
    await waitFor(() => expect(onDismissed).toHaveBeenCalledWith("notice-1"));
  });

  it("renders the presented error key when NoticeNotFound is returned (F27)", async () => {
    vi.mocked(gateway.dismissConflictNotice).mockResolvedValue({
      status: "error",
      error: { code: "NoticeNotFound", notice_id: "notice-1" },
    });
    const notices = [makeNotice({ notice_id: "notice-1" })];

    const { result } = renderHook(() => useNoticeList({ notices, onDismissed: vi.fn() }));

    await act(async () => {
      await result.current.handleDismiss("notice-1");
    });

    expect(result.current.dismissError).toEqual({ key: "sync.errors.NoticeNotFound" });
  });
});

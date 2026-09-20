import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncFolderState, SyncStatus } from "@/bindings";

// 1. Mock the gateway module before importing the hook (test_convention.md § Mocking gateway modules)
vi.mock("../../gateway", () => ({
  pickSyncFolder: vi.fn(),
  inspectSyncFolder: vi.fn(),
  enableSync: vi.fn(),
  startSyncOver: vi.fn(),
}));

// 2. Import mocked modules for typed access
import * as gateway from "../../gateway";
import { useEnableSyncModal } from "./useEnableSyncModal";

function makeSyncStatus(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    enabled: true,
    paused: false,
    device_id: "device-1",
    device_name: "Desktop",
    folder: "/home/user/sync",
    app_version: "0.43.0",
    last_sync_completed_at: null,
    roster: [],
    held_back_count: 0,
    oldest_held_back_since: null,
    notices: [],
    inconsistent_holdings: [],
    failures: [],
    ...overrides,
  };
}

function makeFolderState(overrides: Partial<SyncFolderState> = {}): SyncFolderState {
  return {
    problem: null,
    holds_portfolio: false,
    data_format_version: null,
    format_readable: true,
    installation_holds_user_data: false,
    ...overrides,
  };
}

describe("useEnableSyncModal — folder step (SYN-011/019)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("fills the folder field from pickSyncFolder (D11)", async () => {
    vi.mocked(gateway.pickSyncFolder).mockResolvedValue("/home/user/chosen");
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState(),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));

    await act(async () => {
      await result.current.handleBrowse();
    });

    expect(result.current.folder).toBe("/home/user/chosen");
  });

  it("calls inspectSyncFolder when the folder changes", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState(),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));

    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    expect(gateway.inspectSyncFolder).toHaveBeenCalledWith("/home/user/sync");
  });

  it("blocks step 2 and shows a folder error when the folder reports a problem (SYN-019/069)", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ problem: "PermissionDenied" }),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    expect(result.current.canProceedToStep2).toBe(false);
    expect(result.current.folderError).toEqual({
      key: "sync.folder_problem.PermissionDenied",
    });
  });

  it("uses first-device wording and allows step 2 when holds_portfolio is false", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ holds_portfolio: false }),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    expect(result.current.isJoin).toBe(false);
    expect(result.current.canProceedToStep2).toBe(true);
  });

  it("blocks step 2 with the fresh-install message when installation_holds_user_data is true (SYN-014)", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ holds_portfolio: true, installation_holds_user_data: true }),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    expect(result.current.isJoin).toBe(true);
    expect(result.current.canProceedToStep2).toBe(false);
    expect(result.current.folderError).toEqual({ key: "sync.errors.InstallationHoldsUserData" });
  });

  it("blocks step 2 with the update message when format_readable is false (SYN-019/035)", async () => {
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({
        holds_portfolio: true,
        format_readable: false,
        data_format_version: 9,
      }),
    });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    expect(result.current.canProceedToStep2).toBe(false);
    expect(result.current.folderError).toEqual({
      key: "sync.errors.UpdateRequired",
      vars: { dataFormatVersion: 9 },
    });
  });
});

describe("useEnableSyncModal — stale folder inspections", () => {
  beforeEach(() => vi.clearAllMocks());

  it("ignores the response of an earlier folder once a newer one was typed", async () => {
    let resolveFirst!: (value: { status: "ok"; data: SyncFolderState }) => void;
    vi.mocked(gateway.inspectSyncFolder)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValueOnce({ status: "ok", data: makeFolderState({ problem: "Missing" }) });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    let firstInspection!: Promise<void>;
    act(() => {
      firstInspection = result.current.setFolder("/home/user/old");
    });
    await act(async () => {
      await result.current.setFolder("/home/user/new");
    });
    expect(result.current.folderError).toEqual({ key: "sync.folder_problem.Missing" });

    await act(async () => {
      resolveFirst({ status: "ok", data: makeFolderState() });
      await firstInspection;
    });

    expect(result.current.folderError).toEqual({ key: "sync.folder_problem.Missing" });
    expect(result.current.canProceedToStep2).toBe(false);
  });
});

describe("useEnableSyncModal — passphrase step (SYN-011/012/015)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ holds_portfolio: false }),
    });
  });

  it("blocks submission when the passphrase is shorter than 12 characters (SYN-012)", async () => {
    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    act(() => result.current.setPassphrase("short"));

    expect(result.current.passphraseTooShort).toBe(true);
    expect(result.current.canSubmit).toBe(false);
  });

  it("never blocks on the advisory strength hint alone", async () => {
    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    act(() => {
      result.current.setPassphrase("aaaaaaaaaaaa"); // 12 chars, weak but long enough
      result.current.setPassphraseConfirm("aaaaaaaaaaaa");
      result.current.setDeviceName("Desktop");
    });

    expect(result.current.passphraseTooShort).toBe(false);
    expect(result.current.canSubmit).toBe(true);
  });

  it("blocks submission on a first-device passphrase mismatch (SYN-011)", async () => {
    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    act(() => {
      result.current.setPassphrase("correct horse battery");
      result.current.setPassphraseConfirm("correct horse staple");
      result.current.setDeviceName("Desktop");
    });

    expect(result.current.passphraseMismatch).toBe(true);
    expect(result.current.canSubmit).toBe(false);
  });

  it("requires a non-blank device name (SYN-018)", async () => {
    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });

    act(() => {
      result.current.setPassphrase("correct horse battery");
      result.current.setPassphraseConfirm("correct horse battery");
      result.current.setDeviceName("   ");
    });

    expect(result.current.canSubmit).toBe(false);
  });
});

describe("useEnableSyncModal — submit (SYN-011)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ holds_portfolio: false }),
    });
  });

  it("calls enableSync with positional folder/passphrase/deviceName on submit", async () => {
    vi.mocked(gateway.enableSync).mockResolvedValue({ status: "ok", data: makeSyncStatus() });
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useEnableSyncModal({ variant: "enable", onSuccess }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });
    act(() => {
      result.current.setPassphrase("correct horse battery");
      result.current.setPassphraseConfirm("correct horse battery");
      result.current.setDeviceName("Desktop");
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(gateway.enableSync).toHaveBeenCalledWith(
      "/home/user/sync",
      "correct horse battery",
      "Desktop",
    );
    await waitFor(() => expect(onSuccess).toHaveBeenCalled());
  });
});

describe("useEnableSyncModal — start-over variant (SYN-071)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(gateway.inspectSyncFolder).mockResolvedValue({
      status: "ok",
      data: makeFolderState({ holds_portfolio: false }),
    });
  });

  it("does not call startSyncOver until its own confirmation is accepted", async () => {
    const { result } = renderHook(() => useEnableSyncModal({ variant: "start-over" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });
    act(() => {
      result.current.setPassphrase("correct horse battery");
      result.current.setPassphraseConfirm("correct horse battery");
      result.current.setDeviceName("Desktop");
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(result.current.confirmingStartOver).toBe(true);
    expect(gateway.startSyncOver).not.toHaveBeenCalled();
  });

  it("calls startSyncOver only after the start-over confirmation is accepted", async () => {
    vi.mocked(gateway.startSyncOver).mockResolvedValue({ status: "ok", data: makeSyncStatus() });

    const { result } = renderHook(() => useEnableSyncModal({ variant: "start-over" }));
    await act(async () => {
      await result.current.setFolder("/home/user/sync");
    });
    act(() => {
      result.current.setPassphrase("correct horse battery");
      result.current.setPassphraseConfirm("correct horse battery");
      result.current.setDeviceName("Desktop");
    });
    await act(async () => {
      await result.current.handleSubmit();
    });

    await act(async () => {
      await result.current.confirmStartOver();
    });

    expect(gateway.startSyncOver).toHaveBeenCalledWith(
      "/home/user/sync",
      "correct horse battery",
      "Desktop",
    );
  });
});

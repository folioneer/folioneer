import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

let changed: (() => void) | null = null;
const unlisten = vi.fn();
vi.mock("../gateway", () => ({
  getAgentConnectionState: vi.fn(),
  answerAgentConnection: vi.fn(),
  answerRemovalRequest: vi.fn(),
  answerChangeRequest: vi.fn(),
  answerReplacementRequest: vi.fn(),
  disconnectAgent: vi.fn(),
  removeAgentRecordings: vi.fn(),
  onAgentConnectionChanged: vi.fn((callback: () => void) => {
    changed = callback;
    return Promise.resolve(unlisten);
  }),
}));

const showSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => showSnackbar }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, number>) =>
      vars ? `${key}(${Object.values(vars).join(",")})` : key,
  }),
}));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

import * as gateway from "../gateway";
import { useAgentConnections } from "./useAgentConnections";

const request = (id: number) => ({ id, client: `agent-${id}`, asked_at: "2026-10-09T14:32:00" });
const session = (id: number) => ({ id, client: `agent-${id}`, calls: 2 });
const state = (overrides = {}) => ({
  available: true,
  allowed: true,
  user: "phil",
  requests: [],
  sessions: [],
  removals: [],
  changes: [],
  replacements: [],
  ...overrides,
});

describe("useAgentConnections", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    changed = null;
  });

  // AGT-036 — the window shows what the core holds: read at mount and after every change,
  // the oldest request first.
  it("reads who asks and who is connected, again after every change", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ requests: [request(1), request(2)] }),
    );
    const { result, unmount } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(result.current.request).toEqual(request(1)));
    expect(result.current.user).toBe("phil");
    expect(result.current.sessions).toEqual([]);

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state({ sessions: [session(1)] }));
    act(() => changed?.());
    await waitFor(() => expect(result.current.sessions).toEqual([session(1)]));
    expect(result.current.request).toBeNull();

    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  // AGT-032 / AGT-034 — the owner's answer and disconnect go to the core, and the state is
  // read again.
  it("sends the owner's answer and disconnect", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    vi.mocked(gateway.answerAgentConnection).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(gateway.disconnectAgent).mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalledTimes(1));

    act(() => result.current.answer(7, false));
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalledTimes(2));
    expect(gateway.answerAgentConnection).toHaveBeenCalledWith(7, false);

    act(() => result.current.disconnect(3));
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalledTimes(3));
    expect(gateway.disconnectAgent).toHaveBeenCalledWith(3);
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  // A read that answers after a later one is not shown.
  it("shows only the latest read", async () => {
    let answerFirst: (value: ReturnType<typeof state>) => void = () => {};
    vi.mocked(gateway.getAgentConnectionState).mockReturnValueOnce(
      new Promise((resolve) => {
        answerFirst = resolve;
      }),
    );
    const { result } = renderHook(() => useAgentConnections());

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state({ sessions: [session(2)] }));
    act(() => changed?.());
    await waitFor(() => expect(result.current.sessions).toEqual([session(2)]));

    await act(async () => answerFirst(state({ sessions: [session(1)] })));
    expect(result.current.sessions).toEqual([session(2)]);
  });

  it("says why when the request or the session is already gone", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    vi.mocked(gateway.answerAgentConnection).mockResolvedValue({
      status: "error",
      error: { code: "ConnectionRequestGone" },
    });
    vi.mocked(gateway.disconnectAgent).mockRejectedValue(new Error("ipc"));
    const { result } = renderHook(() => useAgentConnections());

    act(() => result.current.answer(7, true));
    await waitFor(() =>
      expect(showSnackbar).toHaveBeenCalledWith("error.ConnectionRequestGone", "error"),
    );
    act(() => result.current.disconnect(3));
    await waitFor(() => expect(showSnackbar).toHaveBeenCalledWith("error.Unknown", "error"));
  });

  // AGT-053 — the owner's removal goes to the core; what it answers is said, and the state
  // is read again.
  it("removes what a session recorded and says how much went", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    vi.mocked(gateway.removeAgentRecordings).mockResolvedValue({
      status: "ok",
      data: { removed: 12, kept: 0 },
    });
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalledTimes(1));

    act(() => result.current.removeRecordings(3));

    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalledTimes(2));
    expect(gateway.removeAgentRecordings).toHaveBeenCalledWith(3);
    expect(showSnackbar).toHaveBeenCalledWith("agent.session_removed(12)", "success");
  });

  // AGT-053 — what could not be removed is said, as an error.
  it("says how many recordings were kept", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    vi.mocked(gateway.removeAgentRecordings).mockResolvedValue({
      status: "ok",
      data: { removed: 10, kept: 2 },
    });
    const { result } = renderHook(() => useAgentConnections());

    act(() => result.current.removeRecordings(3));

    await waitFor(() =>
      expect(showSnackbar).toHaveBeenCalledWith(
        "agent.session_removed(10) agent.session_kept(2)",
        "error",
      ),
    );
  });

  // AGT-039 / AGT-053 — a removal the core refuses, or that fails, is said too.
  it("says why a removal was refused or failed", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    vi.mocked(gateway.removeAgentRecordings).mockResolvedValue({
      status: "error",
      error: { code: "SessionAlreadyEnded" },
    });
    const { result } = renderHook(() => useAgentConnections());

    act(() => result.current.removeRecordings(3));
    await waitFor(() =>
      expect(showSnackbar).toHaveBeenCalledWith("error.SessionAlreadyEnded", "error"),
    );

    vi.mocked(gateway.removeAgentRecordings).mockRejectedValue(new Error("no core"));
    act(() => result.current.removeRecordings(3));
    await waitFor(() => expect(showSnackbar).toHaveBeenCalledWith("error.Unknown", "error"));
  });
  // AGT-083 — a removal request is answered once no agent waits to connect, oldest first.
  it("offers the oldest removal request once no agent waits to connect", async () => {
    const removal = (id: number) => ({ id, session: 1, client: "agent-1" });
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ requests: [request(1)], removals: [removal(7), removal(8)] }) as never,
    );
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(result.current.request).toEqual(request(1)));
    expect(result.current.removal).toBeNull();

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ removals: [removal(7), removal(8)] }) as never,
    );
    act(() => changed?.());
    await waitFor(() => expect(result.current.removal).toEqual(removal(7)));
  });

  // AGT-088 / AGT-089 — removal and change requests share one order: the oldest of either
  // is offered, once no agent waits to connect.
  it("offers the oldest removal or change request, one at a time", async () => {
    const removal = (id: number) => ({ id, session: 1, client: "agent-1" });
    const change = (id: number) => ({ id, session: 2, client: "agent-2" });
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ removals: [removal(8)], changes: [change(5), change(9)] }) as never,
    );
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(result.current.change).toEqual(change(5)));
    expect(result.current.removal).toBeNull();

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ removals: [removal(8)], changes: [change(9)] }) as never,
    );
    act(() => changed?.());
    await waitFor(() => expect(result.current.removal).toEqual(removal(8)));
    expect(result.current.change).toBeNull();

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ requests: [request(1)], changes: [change(9)] }) as never,
    );
    act(() => changed?.());
    await waitFor(() => expect(result.current.request).toEqual(request(1)));
    expect(result.current.change).toBeNull();
  });

  // AGT-091 — replacement requests join the one order: the oldest of the three kinds is
  // offered, the others wait their turn.
  it("offers the oldest of the removal, change and replacement requests", async () => {
    const asked = (id: number) => ({ id, session: 1, client: "agent-1" });
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ removals: [asked(8)], changes: [asked(6)], replacements: [asked(4)] }) as never,
    );
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(result.current.replacement).toEqual(asked(4)));
    expect(result.current.removal).toBeNull();
    expect(result.current.change).toBeNull();

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ removals: [asked(8)], replacements: [asked(9)] }) as never,
    );
    act(() => changed?.());
    await waitFor(() => expect(result.current.removal).toEqual(asked(8)));
    expect(result.current.replacement).toBeNull();

    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(
      state({ requests: [request(1)], replacements: [asked(9)] }) as never,
    );
    act(() => changed?.());
    await waitFor(() => expect(result.current.request).toEqual(request(1)));
    expect(result.current.replacement).toBeNull();
  });

  // AGT-092 — the owner's answer to a replacement is sent; one that did not happen says why.
  it("sends the owner's answer and says why a replacement did not happen", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalled());

    for (const [error, sentence] of [
      [
        { code: "TransactionsNotReplaced", reason: "TransactionChanged" },
        "agent.replacement_changed",
      ],
      [
        { code: "TransactionsNotReplaced", reason: "TransactionNotFound" },
        "agent.replacement_gone",
      ],
      [
        { code: "TransactionsNotReplaced", reason: "InsufficientCash" },
        "error.TransactionsNotReplaced",
      ],
      [{ code: "ReplacementRequestGone" }, "error.ReplacementRequestGone"],
      [{ code: "DatabaseError" }, "error.DatabaseError"],
    ] as const) {
      vi.mocked(gateway.answerReplacementRequest).mockResolvedValue({
        status: "error",
        error,
      } as never);
      act(() => result.current.answerReplacement(3, true));
      await waitFor(() => expect(showSnackbar).toHaveBeenLastCalledWith(sentence, "error"));
    }
    expect(gateway.answerReplacementRequest).toHaveBeenLastCalledWith(3, true);
  });

  // AGT-088 — the owner's answer to a change is sent; a change that did not happen says why.
  it("sends the owner's answer and says why a change did not happen", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalled());

    for (const [error, sentence] of [
      [{ code: "TransactionNotChanged", reason: "TransactionChanged" }, "agent.change_changed"],
      [{ code: "TransactionNotChanged", reason: "TransactionNotFound" }, "agent.change_gone"],
      [
        { code: "TransactionNotChanged", reason: "CascadingOversell" },
        "error.TransactionNotChanged",
      ],
      [{ code: "ChangeRequestGone" }, "error.ChangeRequestGone"],
    ] as const) {
      vi.mocked(gateway.answerChangeRequest).mockResolvedValue({
        status: "error",
        error,
      } as never);
      act(() => result.current.answerChange(5, true));
      await waitFor(() => expect(showSnackbar).toHaveBeenLastCalledWith(sentence, "error"));
    }
    expect(gateway.answerChangeRequest).toHaveBeenLastCalledWith(5, true);
  });

  // AGT-081 — the owner's answer is sent; a removal that did not happen says why.
  it("sends the owner's answer and says why a removal did not happen", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state());
    const { result } = renderHook(() => useAgentConnections());
    await waitFor(() => expect(gateway.getAgentConnectionState).toHaveBeenCalled());

    for (const [error, sentence] of [
      [{ code: "TransactionNotRemoved", reason: "TransactionChanged" }, "agent.removal_changed"],
      [{ code: "TransactionNotRemoved", reason: "TransactionNotFound" }, "agent.removal_gone"],
      [
        { code: "TransactionNotRemoved", reason: "CascadingOversell" },
        "error.TransactionNotRemoved",
      ],
      [{ code: "RemovalRequestGone" }, "error.RemovalRequestGone"],
    ] as const) {
      vi.mocked(gateway.answerRemovalRequest).mockResolvedValue({
        status: "error",
        error,
      } as never);
      act(() => result.current.answerRemoval(7, true));
      await waitFor(() => expect(showSnackbar).toHaveBeenLastCalledWith(sentence, "error"));
    }
    expect(gateway.answerRemovalRequest).toHaveBeenLastCalledWith(7, true);
  });
});

import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentSession } from "@/bindings";

const showSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => showSnackbar }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, string>) =>
      vars ? `${key}(${Object.values(vars).join("|")})` : key,
    i18n: { language: "fr" },
  }),
}));

import { useAgentRecordingNotice } from "./useAgentRecordingNotice";

const session = (overrides: Partial<AgentSession> = {}): AgentSession => ({
  id: 1,
  client: "Claude Code",
  calls: 3,
  started_at: "2026-10-14T09:00:00+02:00",
  reads: 1,
  recordings: 1,
  transactions_recorded: 1,
  last_recording: { kind: "Purchase", asset: "CW8", date: "2019-03-14" },
  accounts_added: 0,
  last_account: null,
  assets_added: 0,
  last_asset: null,
  ...overrides,
});

function rendered(first: AgentSession[]) {
  return renderHook(({ sessions }) => useAgentRecordingNotice(sessions), {
    initialProps: { sessions: first },
  });
}

describe("useAgentRecordingNotice", () => {
  beforeEach(() => vi.clearAllMocks());

  // AGT-084 — a session first read is remembered, not announced.
  it("announces nothing at the first read", () => {
    rendered([session({ recordings: 5 })]);
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  // AGT-084 — each new recording is said with the agent, its kind, its asset and its date.
  it("announces a new recording with the agent, its kind, its asset and its date", () => {
    const { rerender } = rendered([session()]);
    rerender({
      sessions: [
        session({
          recordings: 2,
          transactions_recorded: 2,
          last_recording: { kind: "Sell", asset: "AI", date: "2019-04-02" },
        }),
      ],
    });
    expect(showSnackbar).toHaveBeenCalledWith(
      "agent.notice_recorded(Claude Code|transaction.type_sell|AI|02/04/2019)",
      "info",
    );
  });

  // AGT-084 — an added asset is said with its name and reference.
  it("announces an added asset", () => {
    const { rerender } = rendered([session()]);
    rerender({
      sessions: [
        session({ assets_added: 1, last_asset: { name: "Air Liquide", reference: "AI" } }),
      ],
    });
    expect(showSnackbar).toHaveBeenCalledWith(
      "agent.notice_added(Claude Code|Air Liquide|AI)",
      "info",
    );
  });

  // AGT-084 — a correction or a cancelling is not announced as a new recording.
  it("announces neither a correction nor a cancelling", () => {
    const { rerender } = rendered([session({ recordings: 2, transactions_recorded: 2 })]);
    rerender({
      sessions: [
        session({
          recordings: 2,
          transactions_recorded: 2,
          last_recording: { kind: "Purchase", asset: "CW8", date: "2019-03-15" },
        }),
      ],
    });
    rerender({ sessions: [session({ recordings: 1, transactions_recorded: 2 })] });
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  // AGT-084 — a recording made after one was removed is announced, though what still
  // exists did not rise.
  it("announces a recording made after one was removed", () => {
    const { rerender } = rendered([session({ recordings: 1, transactions_recorded: 1 })]);
    rerender({
      sessions: [session({ recordings: 0, transactions_recorded: 1, last_recording: null })],
    });
    rerender({ sessions: [session({ recordings: 1, transactions_recorded: 2 })] });
    expect(showSnackbar).toHaveBeenCalledTimes(1);
  });

  // AGT-084 — a session that opens while the window watches is announced from its first.
  it("announces the first recording of a session that opened since", () => {
    const { rerender } = rendered([]);
    rerender({
      sessions: [session({ id: 2, recordings: 0, transactions_recorded: 0, last_recording: null })],
    });
    rerender({ sessions: [session({ id: 2 })] });
    expect(showSnackbar).toHaveBeenCalledTimes(1);
  });

  // AGT-084 — a rise with no recording left to name says nothing.
  it("says nothing of a rise when no recording is left", () => {
    const { rerender } = rendered([session()]);
    rerender({
      sessions: [session({ recordings: 0, transactions_recorded: 2, last_recording: null })],
    });
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  // AGT-084 — a recording and an added asset in one read: the recording first, then the
  // asset, each message replacing the one before.
  it("says a session's recording before its added asset in one read", () => {
    const { rerender } = rendered([session()]);
    rerender({
      sessions: [
        session({
          transactions_recorded: 2,
          assets_added: 1,
          last_asset: { name: "Air Liquide", reference: "AI" },
        }),
      ],
    });
    expect(showSnackbar.mock.calls.map(([message]) => message)).toEqual([
      "agent.notice_recorded(Claude Code|transaction.type_purchase|CW8|14/03/2019)",
      "agent.notice_added(Claude Code|Air Liquide|AI)",
    ]);
  });

  // AGT-084 — several sessions are said in the order the state gives them, oldest first,
  // so the last one said stays.
  it("says several sessions in the order the state gives them", () => {
    const other = (overrides: Partial<AgentSession> = {}) =>
      session({ id: 2, client: "Claude Desktop", ...overrides });
    const { rerender } = rendered([session(), other()]);
    rerender({
      sessions: [session({ transactions_recorded: 2 }), other({ transactions_recorded: 2 })],
    });
    expect(showSnackbar.mock.calls.map(([message]) => message)).toEqual([
      "agent.notice_recorded(Claude Code|transaction.type_purchase|CW8|14/03/2019)",
      "agent.notice_recorded(Claude Desktop|transaction.type_purchase|CW8|14/03/2019)",
    ]);
  });

  // AGT-084 — several recordings between two reads make one message, about the last.
  it("says several recordings between two reads once, about the last", () => {
    const { rerender } = rendered([session()]);
    rerender({
      sessions: [
        session({
          recordings: 4,
          transactions_recorded: 4,
          last_recording: { kind: "Dividend", asset: "AI", date: "2019-05-02" },
        }),
      ],
    });
    expect(showSnackbar).toHaveBeenCalledTimes(1);
    expect(showSnackbar).toHaveBeenCalledWith(
      "agent.notice_recorded(Claude Code|transaction.type_dividend|AI|02/05/2019)",
      "info",
    );
  });
});

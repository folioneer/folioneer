import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, string>) =>
      vars ? `${key}(${Object.values(vars).join(",")})` : key,
    i18n: { language: "fr" },
  }),
}));

const hook = vi.fn();
vi.mock("./useAgentConnections", () => ({ useAgentConnections: () => hook() }));

import { ALLOW_AFTER_MS } from "./AgentConnectionDialog";
import { AgentConnectionMount } from "./AgentConnectionMount";
import { AgentIndicator } from "./AgentIndicator";

const answer = vi.fn();
const answerRemoval = vi.fn();
const disconnect = vi.fn();
const removeRecordings = vi.fn();
const state = (overrides = {}) => ({
  request: null,
  sessions: [],
  removal: null,
  user: "phil",
  answer,
  answerRemoval,
  disconnect,
  removeRecordings,
  ...overrides,
});
const session = (overrides = {}) => ({
  id: 1,
  client: "Claude Code",
  calls: 39,
  started_at: "2026-10-09T14:32:00",
  reads: 27,
  recordings: 12,
  last_recording: { kind: "Purchase", asset: "CW8", date: "2019-12-31" },
  assets_added: 0,
  last_asset: null,
  ...overrides,
});
const request = { id: 4, client: "Claude Code", asked_at: "2026-10-09T14:32:00" };
const removal = (overrides = {}) => ({
  id: 9,
  session: 1,
  client: "Claude Code",
  asked_at: "2026-10-14T09:00:00",
  transaction: {
    id: "tx-1",
    date: "2026-01-02",
    account: "PEA Boursorama",
    currency: "EUR",
    kind: "OpeningBalance",
    asset_name: "Amundi MSCI World",
    asset_reference: "CW8",
    quantity: 118_000_000,
    amount: 31_204_000_000,
  },
  ...overrides,
});

describe("AgentConnectionMount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it("shows nothing while no agent asks", () => {
    hook.mockReturnValue(state());
    const { container } = render(<AgentConnectionMount />);
    expect(container).toBeEmptyDOMElement();
  });

  // AGT-032 — the dialog names the agent, who started it and when, and says what a yes
  // covers; refusing is the action in focus.
  it("names the agent and what a yes covers, with refusing in focus", () => {
    hook.mockReturnValue(state({ request }));
    render(<AgentConnectionMount />);

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("id", "agent-connection-dialog");
    expect(dialog).toHaveTextContent("Claude Code");
    expect(dialog).toHaveTextContent("agent.connection_started_by_user(phil)");
    expect(dialog).toHaveTextContent("14:32");
    expect(dialog).toHaveTextContent("agent.connection_covers_read");
    // AGT-064 — adding assets has a line of its own, between reading and recording.
    expect(Array.from(dialog.querySelectorAll("li")).map((line) => line.textContent)).toEqual([
      "agent.connection_covers_read",
      "agent.connection_covers_add",
      "agent.connection_covers_record",
    ]);
    expect(dialog).toHaveTextContent("agent.connection_never");
    expect(document.getElementById("agent-connection-refuse")).toHaveFocus();
  });

  // AGT-032 — the dialog closes only by an answer: no corner button, and each action sends
  // its answer for this request.
  it("closes only by an answer", () => {
    hook.mockReturnValue(state({ request, user: null }));
    render(<AgentConnectionMount />);

    expect(document.getElementById("modal-close-btn")).toBeNull();
    expect(screen.getByRole("dialog")).toHaveTextContent("agent.connection_started_by_unknown");

    fireEvent.click(screen.getByText("agent.connection_refuse"));
    expect(answer).toHaveBeenLastCalledWith(4, false);
    act(() => vi.advanceTimersByTime(ALLOW_AFTER_MS));
    fireEvent.click(screen.getByText("agent.connection_allow"));
    expect(answer).toHaveBeenLastCalledWith(4, true);
  });

  // AGT-032 — hard to approve blindly: a dialog that just appeared allows nothing, also
  // when it replaces another one under the pointer.
  it("keeps Allow inactive for a moment after it appears, for each request anew", () => {
    hook.mockReturnValue(state({ request }));
    const { rerender } = render(<AgentConnectionMount />);
    const allow = () => document.getElementById("agent-connection-allow") as HTMLButtonElement;

    expect(allow()).toBeDisabled();
    fireEvent.click(allow());
    expect(answer).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(ALLOW_AFTER_MS));
    expect(allow()).toBeEnabled();

    hook.mockReturnValue(state({ request: { ...request, id: 5, client: "Other" } }));
    rerender(<AgentConnectionMount />);
    expect(allow()).toBeDisabled();
    expect(document.getElementById("agent-connection-refuse")).toHaveFocus();
  });

  // AGT-032 — the keyboard stays on the dialog's actions: Tab never reaches what lies
  // behind it.
  it("keeps Tab on the dialog's actions", () => {
    hook.mockReturnValue(state({ request }));
    render(
      <>
        <button type="button" id="behind">
          behind
        </button>
        <AgentConnectionMount />
      </>,
    );
    const refuse = document.getElementById("agent-connection-refuse") as HTMLElement;
    const allow = document.getElementById("agent-connection-allow") as HTMLElement;

    fireEvent.keyDown(document.activeElement as Element, { key: "Tab" });
    expect(refuse).toHaveFocus();

    act(() => vi.advanceTimersByTime(ALLOW_AFTER_MS));
    fireEvent.keyDown(refuse, { key: "Tab" });
    expect(allow).toHaveFocus();
    fireEvent.keyDown(allow, { key: "Tab" });
    expect(refuse).toHaveFocus();
    fireEvent.keyDown(refuse, { key: "Tab", shiftKey: true });
    expect(allow).toHaveFocus();
    expect(document.getElementById("behind")).not.toHaveFocus();
  });
  // AGT-083 — an agent asks to remove a transaction the owner typed: the transaction in
  // full, what would become of it, and that the answer covers it alone; refusing is in
  // focus, and removing is inactive for a moment.
  it("shows a removal request in full, with refusing in focus", () => {
    hook.mockReturnValue(state({ removal: removal() }));
    render(<AgentConnectionMount />);

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("id", "agent-removal-dialog");
    expect(dialog).toHaveTextContent("agent.removal_title(Claude Code)");
    expect(dialog).toHaveTextContent("02/01/2026");
    expect(dialog).toHaveTextContent("PEA Boursorama");
    expect(dialog).toHaveTextContent("transaction.type_openingbalance");
    expect(dialog).toHaveTextContent("Amundi MSCI World (CW8)");
    expect(dialog).toHaveTextContent("118");
    expect(dialog).toHaveTextContent("EUR");
    expect(dialog).toHaveTextContent("agent.removal_becomes_removed");
    expect(dialog).toHaveTextContent("agent.removal_covers");
    expect(document.getElementById("modal-close-btn")).toBeNull();
    expect(document.getElementById("agent-removal-refuse")).toHaveFocus();
    const remove = document.getElementById("agent-removal-remove") as HTMLButtonElement;
    expect(remove).toBeDisabled();

    fireEvent.click(screen.getByText("agent.removal_refuse"));
    expect(answerRemoval).toHaveBeenLastCalledWith(9, false);
    act(() => vi.advanceTimersByTime(ALLOW_AFTER_MS));
    fireEvent.click(remove);
    expect(answerRemoval).toHaveBeenLastCalledWith(9, true);
  });

  // AGT-083 — a transaction on the account's cash has no asset and no quantity to show.
  it("shows no asset for a transaction on the account's cash", () => {
    const cash = removal().transaction;
    hook.mockReturnValue(
      state({
        removal: removal({
          transaction: { ...cash, kind: "Deposit", asset_name: null, asset_reference: null },
        }),
      }),
    );
    render(<AgentConnectionMount />);

    const dialog = screen.getByRole("dialog");
    expect(dialog).not.toHaveTextContent("agent.removal_asset");
    expect(dialog).not.toHaveTextContent("agent.removal_quantity");
    expect(dialog).toHaveTextContent("agent.removal_amount");
  });

  // AGT-083 — an agent asking to connect comes first; the removal waits its turn.
  it("shows a connection request before a removal request", () => {
    hook.mockReturnValue(state({ request, removal: null }));
    render(<AgentConnectionMount />);
    expect(screen.getByRole("dialog")).toHaveAttribute("id", "agent-connection-dialog");
  });
});

describe("AgentIndicator", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows nothing while no agent is connected", () => {
    hook.mockReturnValue(state());
    const { container } = render(<AgentIndicator />);
    expect(container).toBeEmptyDOMElement();
  });

  // AGT-033 / AGT-034 — the header names each connected agent and disconnects the one asked.
  it("names each connected agent and disconnects the one asked", () => {
    hook.mockReturnValue(
      state({
        sessions: [session(), session({ id: 2, client: "Claude Desktop" })],
      }),
    );
    render(<AgentIndicator />);

    expect(document.getElementById("agent-connected-1")).toHaveTextContent(
      "agent.connected(Claude Code)",
    );
    expect(document.getElementById("agent-connected-2")).toHaveTextContent(
      "agent.connected(Claude Desktop)",
    );

    expect(document.getElementById("agent-connected-2")).toHaveAttribute("title", "Claude Desktop");
    const second = document.getElementById("agent-disconnect-2") as HTMLElement;
    expect(second).toHaveAccessibleName("agent.disconnect_client(Claude Desktop)");

    fireEvent.click(second);
    expect(disconnect).toHaveBeenCalledWith(2);
  });

  // AGT-052 — the header's agent opens what its session did: since when, how much it
  // read and recorded, and its last recording.
  it("opens what the session did from the agent's name", () => {
    hook.mockReturnValue(state({ sessions: [session()] }));
    render(<AgentIndicator />);
    expect(screen.queryByRole("dialog")).toBeNull();
    const opener = document.getElementById("agent-connected-1") as HTMLElement;
    expect(opener).toHaveAccessibleName("agent.session_open(Claude Code)");

    fireEvent.click(opener);

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("id", "agent-session-dialog");
    expect(dialog).toHaveTextContent("agent.connected(Claude Code)");
    expect(dialog).toHaveTextContent("14:32");
    expect(dialog).toHaveTextContent("agent.session_read_count(27)");
    expect(dialog).toHaveTextContent("agent.session_recorded_count(12)");
    expect(dialog).toHaveTextContent("transaction.type_purchase · CW8 · 31/12/2019");
    expect(document.getElementById("agent-session-note")).toHaveTextContent(
      "agent.session_remove_note(12)",
    );

    fireEvent.click(document.getElementById("agent-session-close") as HTMLElement);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  // AGT-064 — the session dialog says how many assets the session added and names the
  // last one, between what it read and what it recorded; none is said as such.
  it("says the assets the session added, or that it added none", () => {
    hook.mockReturnValue(
      state({
        sessions: [
          session({ assets_added: 2, last_asset: { name: "Air Liquide", reference: "AI" } }),
        ],
      }),
    );
    const { unmount } = render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);
    const rows = () =>
      Array.from(screen.getByRole("dialog").querySelectorAll("dt")).map((row) => [
        row.textContent,
        row.nextElementSibling?.textContent,
      ]);
    expect(rows()[1]).toEqual(["agent.session_read", "agent.session_read_count(27)"]);
    expect(rows()[2]).toEqual([
      "agent.session_assets",
      "agent.session_assets_count(2,Air Liquide,AI)",
    ]);
    expect(rows()[3]).toEqual(["agent.session_recorded", "agent.session_recorded_count(12)"]);
    unmount();

    hook.mockReturnValue(state({ sessions: [session()] }));
    render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);
    expect(rows()[2]).toEqual(["agent.session_assets", "agent.session_none"]);
  });

  // AGT-053 — removing everything asks to confirm, keeping is the action in focus, and
  // only the confirmation removes.
  it("removes everything the session recorded only after a confirmation that keeps by default", () => {
    hook.mockReturnValue(state({ sessions: [session()] }));
    render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);

    fireEvent.click(document.getElementById("agent-session-remove") as HTMLElement);
    const confirmation = screen.getByRole("dialog");
    expect(confirmation).toHaveAttribute("id", "agent-session-remove-dialog");
    expect(confirmation).toHaveTextContent("agent.session_confirm_title(Claude Code)");
    expect(confirmation).toHaveTextContent("agent.session_confirm_message(12)");
    expect(document.getElementById("agent-session-remove-keep")).toHaveFocus();

    fireEvent.click(document.getElementById("agent-session-remove-keep") as HTMLElement);
    expect(removeRecordings).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toHaveAttribute("id", "agent-session-dialog");

    fireEvent.click(document.getElementById("agent-session-remove") as HTMLElement);
    fireEvent.click(document.getElementById("agent-session-remove-confirm") as HTMLElement);
    expect(removeRecordings).toHaveBeenCalledWith(1);
    expect(screen.getByRole("dialog")).toHaveAttribute("id", "agent-session-dialog");
  });

  // AGT-053 — a session with nothing left to remove offers no removal.
  it("offers no removal when the session has no recording left", () => {
    hook.mockReturnValue(state({ sessions: [session({ recordings: 0, last_recording: null })] }));
    render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);

    expect(document.getElementById("agent-session-remove")).toBeDisabled();
    expect(document.getElementById("agent-session-note")).toHaveTextContent(
      "agent.session_nothing",
    );
    expect(screen.getByRole("dialog")).toHaveTextContent("agent.session_none");
  });

  // AGT-032 / AGT-054 — an agent that asks to connect comes first: the session dialog gives
  // way until the request is answered.
  it("gives way to a connection request and comes back once it is answered", () => {
    hook.mockReturnValue(state({ sessions: [session()] }));
    const { rerender } = render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);
    expect(document.getElementById("agent-session-dialog")).not.toBeNull();

    hook.mockReturnValue(state({ sessions: [session()], request }));
    rerender(<AgentIndicator />);
    expect(document.getElementById("agent-session-dialog")).toBeNull();

    hook.mockReturnValue(state({ sessions: [session()] }));
    rerender(<AgentIndicator />);
    expect(document.getElementById("agent-session-dialog")).not.toBeNull();
  });

  // AGT-083 / AGT-054 — an agent that asks to remove a transaction comes first too.
  it("gives way to a removal request and comes back once it is answered", () => {
    hook.mockReturnValue(state({ sessions: [session()] }));
    const { rerender } = render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);

    hook.mockReturnValue(state({ sessions: [session()], removal: removal() }));
    rerender(<AgentIndicator />);
    expect(document.getElementById("agent-session-dialog")).toBeNull();

    hook.mockReturnValue(state({ sessions: [session()] }));
    rerender(<AgentIndicator />);
    expect(document.getElementById("agent-session-dialog")).not.toBeNull();
  });

  // AGT-034 — the dialog disconnects its agent, and closes once the session is gone.
  it("disconnects from the dialog, which closes with its session", () => {
    hook.mockReturnValue(state({ sessions: [session()] }));
    const { rerender } = render(<AgentIndicator />);
    fireEvent.click(document.getElementById("agent-connected-1") as HTMLElement);

    fireEvent.click(document.getElementById("agent-session-disconnect") as HTMLElement);
    expect(disconnect).toHaveBeenCalledWith(1);

    hook.mockReturnValue(state());
    rerender(<AgentIndicator />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

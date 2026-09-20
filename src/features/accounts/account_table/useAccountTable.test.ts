import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, AccountDeletionSummary, AccountSummary } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { useAccountTable } from "./useAccountTable";

function makeAccount(
  id: string,
  name: string,
  freq: AccountSummary["update_frequency"],
  total_global_value = 0,
  total_unrealized_pnl: number | null = null,
  ytd_performance_pct: number | null = null,
): AccountSummary {
  return {
    id,
    name,
    currency: "EUR",
    update_frequency: freq,
    total_global_value,
    total_unrealized_pnl,
    ytd_performance_pct,
    has_inconsistent_holding: false,
  };
}

const accounts: [AccountSummary, AccountSummary, AccountSummary, AccountSummary] = [
  makeAccount("1", "Alpha", "ManualYear"),
  makeAccount("2", "Beta", "Automatic"),
  makeAccount("3", "Gamma", "ManualDay"),
  makeAccount("4", "Delta", "ManualMonth"),
];

const noopDelete = vi.fn().mockResolvedValue({ error: null });
const noopAccountClick = vi.fn();

function makeEmptySummary(): AccountDeletionSummary {
  return { holding_count: 0, transaction_count: 0 };
}

function makeNonEmptySummary(holdings = 2, transactions = 5): AccountDeletionSummary {
  return { holding_count: holdings, transaction_count: transactions };
}

const noopSummary = vi.fn().mockResolvedValue({ data: makeEmptySummary(), error: null });

function makeKeyEvent(key: string): React.KeyboardEvent {
  return { key, preventDefault: vi.fn() } as unknown as React.KeyboardEvent;
}

function makeMouseEvent(): React.MouseEvent {
  return { stopPropagation: vi.fn() } as unknown as React.MouseEvent;
}

describe("useAccountTable", () => {
  // R9 — frequency sorted by logical enum order, not alphabetical label
  it("sorts update_frequency by logical enum order ascending", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => {
      result.current.handleSort("update_frequency");
    });

    const freqs = result.current.sortedAndFilteredAccounts.map((a) => a.update_frequency);
    expect(freqs).toEqual(["Automatic", "ManualDay", "ManualMonth", "ManualYear"]);
  });

  // R9 — descending reverses logical order
  it("sorts update_frequency by logical enum order descending on second click", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("update_frequency"));
    act(() => result.current.handleSort("update_frequency"));

    const freqs = result.current.sortedAndFilteredAccounts.map((a) => a.update_frequency);
    expect(freqs).toEqual(["ManualYear", "ManualMonth", "ManualDay", "Automatic"]);
  });

  // R10 — search active with no match → hasNoSearchResults true
  it("sets hasNoSearchResults when filter is active but no match", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "zzz", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.hasNoSearchResults).toBe(true);
    expect(result.current.isEmpty).toBe(false);
  });

  // R11 — empty list with no filter → isEmpty true
  it("sets isEmpty when list is empty and no search is active", () => {
    const { result } = renderHook(() =>
      useAccountTable([], "", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.isEmpty).toBe(true);
    expect(result.current.hasNoSearchResults).toBe(false);
  });

  // R10 / R11 — empty list with active filter → hasNoSearchResults (not isEmpty)
  it("sets hasNoSearchResults (not isEmpty) when list is empty but filter is active", () => {
    const { result } = renderHook(() =>
      useAccountTable([], "something", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.hasNoSearchResults).toBe(true);
    expect(result.current.isEmpty).toBe(false);
  });

  // ACC-008 / ACC-021 — sort by total_global_value ascending then descending (numeric, not string)
  it("sorts total_global_value numerically ascending then descending", () => {
    const valued: AccountSummary[] = [
      makeAccount("a", "A", "ManualMonth", 500_000_000), // 500
      makeAccount("b", "B", "ManualMonth", 100_000_000), // 100
      makeAccount("c", "C", "ManualMonth", 9_000_000_000), // 9000 — would sort wrong as string
    ];
    const { result } = renderHook(() =>
      useAccountTable(valued, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleSort("total_global_value"));
    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
    act(() => result.current.handleSort("total_global_value"));
    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["c", "a", "b"]);
  });

  // handleGlobalValueKeyDown — Enter triggers sort, Space triggers sort, other keys are ignored
  it("handleGlobalValueKeyDown triggers total_global_value sort on Enter and Space", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.sortConfig.key).toBe("name");

    act(() => result.current.handleGlobalValueKeyDown(makeKeyEvent("Enter")));
    expect(result.current.sortConfig.key).toBe("total_global_value");
    expect(result.current.sortConfig.direction).toBe("asc");

    act(() => result.current.handleGlobalValueKeyDown(makeKeyEvent(" ")));
    expect(result.current.sortConfig.direction).toBe("desc");

    // Any other key is a noop — sort remains where it was.
    act(() => result.current.handleGlobalValueKeyDown(makeKeyEvent("Tab")));
    expect(result.current.sortConfig.direction).toBe("desc");
  });

  // ACC-008 — ties broken by name asc to keep ordering stable
  it("breaks total_global_value ties by name ascending", () => {
    const tied: AccountSummary[] = [
      makeAccount("1", "Beta", "ManualMonth", 100_000_000),
      makeAccount("2", "Alpha", "ManualMonth", 100_000_000),
      makeAccount("3", "Gamma", "ManualMonth", 100_000_000),
    ];
    const { result } = renderHook(() =>
      useAccountTable(tied, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleSort("total_global_value"));
    expect(result.current.sortedAndFilteredAccounts.map((a) => a.name)).toEqual([
      "Alpha",
      "Beta",
      "Gamma",
    ]);
  });

  it("handleRowKeyDown calls onAccountClick on Enter", () => {
    const onClick = vi.fn();
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, onClick),
    );
    const e = makeKeyEvent("Enter");

    act(() => result.current.handleRowKeyDown(e, "2"));

    expect(e.preventDefault).toHaveBeenCalled();
    expect(onClick).toHaveBeenCalledWith("2");
  });

  it("handleRowKeyDown calls onAccountClick on Space", () => {
    const onClick = vi.fn();
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, onClick),
    );
    const e = makeKeyEvent(" ");

    act(() => result.current.handleRowKeyDown(e, "3"));

    expect(onClick).toHaveBeenCalledWith("3");
  });

  it("handleRowKeyDown ignores other keys", () => {
    const onClick = vi.fn();
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, onClick),
    );
    const e = makeKeyEvent("Tab");

    act(() => result.current.handleRowKeyDown(e, "1"));

    expect(e.preventDefault).not.toHaveBeenCalled();
    expect(onClick).not.toHaveBeenCalled();
  });

  it("handleEditClick stops propagation and sets editData", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    const e = makeMouseEvent();
    const account = accounts[0];

    act(() => result.current.handleEditClick(e, account));

    expect(e.stopPropagation).toHaveBeenCalled();
    // editData carries only the bare Account-shape (no total_global_value), so we
    // assert structural equality on those fields rather than reference identity.
    // ACC-026 / FEE-075 — bank_name falls back to "" and the flag to false when
    // the account catalog has no entry.
    expect(result.current.editData).toEqual({
      id: account.id,
      name: account.name,
      bank_name: "",
      currency: account.currency,
      update_frequency: account.update_frequency,
      management_fees_enabled: false,
    });
  });

  it("handleEditClose clears editData", () => {
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleEditClick(makeMouseEvent(), accounts[0]));
    act(() => result.current.handleEditClose());

    expect(result.current.editData).toBeNull();
  });

  // ACC-018 — empty account: handleDeleteClick fetches summary and opens standard dialog
  it("handleDeleteClick fetches summary and sets deleteData (empty account)", async () => {
    const getSummary = vi.fn().mockResolvedValue({ data: makeEmptySummary(), error: null });
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, getSummary, noopAccountClick),
    );
    const e = makeMouseEvent();

    await act(async () => {
      await result.current.handleDeleteClick(e, "1", "Alpha");
    });

    expect(e.stopPropagation).toHaveBeenCalled();
    expect(getSummary).toHaveBeenCalledWith("1");
    expect(result.current.deleteData).toEqual({ id: "1", name: "Alpha" });
    expect(result.current.deleteSummary).toEqual(makeEmptySummary());
  });

  // ACC-019 — non-empty account: summary has holdings, dialog carries counts
  it("handleDeleteClick sets non-empty deleteSummary for reinforced dialog (ACC-019)", async () => {
    const summary = makeNonEmptySummary(3, 7);
    const getSummary = vi.fn().mockResolvedValue({ data: summary, error: null });
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, getSummary, noopAccountClick),
    );

    await act(async () => {
      await result.current.handleDeleteClick(makeMouseEvent(), "2", "Beta");
    });

    expect(result.current.deleteData).toEqual({ id: "2", name: "Beta" });
    expect(result.current.deleteSummary?.holding_count).toBe(3);
    expect(result.current.deleteSummary?.transaction_count).toBe(7);
  });

  // ACC-018/019 — summary fetch error: dialog does not open, actionError is set
  it("handleDeleteClick shows actionError and does not open dialog when summary fetch fails", async () => {
    const getSummary = vi.fn().mockResolvedValue({ data: null, error: "error.Unknown" });
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, getSummary, noopAccountClick),
    );

    await act(async () => {
      await result.current.handleDeleteClick(makeMouseEvent(), "1", "Alpha");
    });

    expect(result.current.deleteData).toBeNull();
    expect(result.current.actionError).toBe("error.Unknown");
  });

  it("handleDeleteCancel clears deleteData and deleteSummary", async () => {
    const getSummary = vi.fn().mockResolvedValue({ data: makeEmptySummary(), error: null });
    const { result } = renderHook(() =>
      useAccountTable(accounts, "", noopDelete, getSummary, noopAccountClick),
    );

    await act(async () => {
      await result.current.handleDeleteClick(makeMouseEvent(), "1", "Alpha");
    });
    act(() => result.current.handleDeleteCancel());

    expect(result.current.deleteData).toBeNull();
    expect(result.current.deleteSummary).toBeNull();
  });
});

// ACC-023 / ACC-024 / ACC-008 — new sort keys: total_unrealized_pnl and ytd_performance_pct
// Both must sort numerically and place nulls LAST in both directions.
describe("useAccountTable — sort by total_unrealized_pnl (ACC-023, ACC-008)", () => {
  // Accounts: one with a positive P&L, one negative, one null
  const pnlAccounts: AccountSummary[] = [
    makeAccount("a", "Alpha", "ManualMonth", 0, 5_000_000, null), // pnl=5, ytd=null
    makeAccount("b", "Beta", "ManualMonth", 0, -2_000_000, null), // pnl=-2, ytd=null
    makeAccount("c", "Gamma", "ManualMonth", 0, null, null), // pnl=null → sorts last
  ];

  it("sorts total_unrealized_pnl ascending, nulls last", () => {
    const { result } = renderHook(() =>
      useAccountTable(pnlAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("total_unrealized_pnl"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
  });

  it("sorts total_unrealized_pnl descending, nulls last", () => {
    const { result } = renderHook(() =>
      useAccountTable(pnlAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("total_unrealized_pnl"));
    act(() => result.current.handleSort("total_unrealized_pnl"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["a", "b", "c"]);
  });

  it("exposes handleUnrealizedPnlKeyDown that triggers sort on Enter", () => {
    const { result } = renderHook(() =>
      useAccountTable(pnlAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.sortConfig.key).toBe("name");

    act(() => result.current.handleUnrealizedPnlKeyDown(makeKeyEvent("Enter")));

    expect(result.current.sortConfig.key).toBe("total_unrealized_pnl");
    expect(result.current.sortConfig.direction).toBe("asc");
  });

  // ACC-008 — when both compared values are null, they order among themselves by name.
  it("orders null total_unrealized_pnl rows among themselves by name", () => {
    const bothNull: AccountSummary[] = [
      makeAccount("z", "Zeta", "ManualMonth", 0, null, null),
      makeAccount("a", "Alpha", "ManualMonth", 0, null, null),
      makeAccount("b", "Beta", "ManualMonth", 0, 5_000_000, null),
    ];
    const { result } = renderHook(() =>
      useAccountTable(bothNull, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleSort("total_unrealized_pnl"));
    // Beta (non-null) first; the two nulls sort last, ordered by name (Alpha < Zeta).
    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "z"]);
  });

  // ACC-008 — equal non-null metrics break ties by name ascending.
  it("breaks equal total_unrealized_pnl ties by name ascending", () => {
    const tied: AccountSummary[] = [
      makeAccount("z", "Zeta", "ManualMonth", 0, 5_000_000, null),
      makeAccount("a", "Alpha", "ManualMonth", 0, 5_000_000, null),
    ];
    const { result } = renderHook(() =>
      useAccountTable(tied, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleSort("total_unrealized_pnl"));
    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["a", "z"]);
  });

  it("handleUnrealizedPnlKeyDown triggers sort on Space and ignores other keys", () => {
    const { result } = renderHook(() =>
      useAccountTable(pnlAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleUnrealizedPnlKeyDown(makeKeyEvent(" ")));
    expect(result.current.sortConfig.key).toBe("total_unrealized_pnl");
    expect(result.current.sortConfig.direction).toBe("asc");

    // Tab is ignored — direction stays asc (a second sort on the same key would flip it).
    act(() => result.current.handleUnrealizedPnlKeyDown(makeKeyEvent("Tab")));
    expect(result.current.sortConfig.direction).toBe("asc");
  });
});

// ACC-026 — bank name resolved from the account catalog; empty string means
// unset and sorts last in both directions (empty-string-as-null semantics).
describe("useAccountTable — bank name column (ACC-026, ACC-008)", () => {
  function makeCatalogAccount(id: string, name: string, bank_name: string): Account {
    return {
      id,
      name,
      bank_name,
      currency: "EUR",
      update_frequency: "ManualMonth",
      management_fees_enabled: false,
    };
  }

  const bankAccounts: [AccountSummary, AccountSummary, AccountSummary] = [
    makeAccount("a", "Alpha", "ManualMonth"),
    makeAccount("b", "Beta", "ManualMonth"),
    makeAccount("c", "Gamma", "ManualMonth"),
  ];

  beforeEach(() => {
    useAppStore.setState({
      accounts: [
        makeCatalogAccount("a", "Alpha", "Fortuneo"),
        makeCatalogAccount("b", "Beta", "Boursorama"),
        makeCatalogAccount("c", "Gamma", ""),
      ],
    });
  });

  afterEach(() => {
    useAppStore.setState({ accounts: [] });
  });

  it("exposes each row's bank name resolved from the account catalog", () => {
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    const bankNamesByAccountId = Object.fromEntries(
      result.current.sortedAndFilteredAccounts.map((a) => [a.id, a.bank_name]),
    );
    expect(bankNamesByAccountId).toEqual({ a: "Fortuneo", b: "Boursorama", c: "" });
  });

  it("falls back to empty string when the catalog has no entry for the account", () => {
    useAppStore.setState({ accounts: [] });
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    expect(result.current.sortedAndFilteredAccounts.every((a) => a.bank_name === "")).toBe(true);
  });

  it("sorts bank_name ascending with empty bank names last", () => {
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("bank_name"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
  });

  it("sorts bank_name descending with empty bank names still last", () => {
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("bank_name"));
    act(() => result.current.handleSort("bank_name"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["a", "b", "c"]);
  });

  it("orders rows with empty bank names among themselves by account name", () => {
    useAppStore.setState({
      accounts: [
        makeCatalogAccount("a", "Alpha", ""),
        makeCatalogAccount("b", "Beta", "Boursorama"),
        makeCatalogAccount("c", "Gamma", ""),
      ],
    });
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("bank_name"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
  });

  it("handleBankNameKeyDown triggers sort on Enter and Space and ignores other keys", () => {
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.sortConfig.key).toBe("name");

    act(() => result.current.handleBankNameKeyDown(makeKeyEvent("Enter")));
    expect(result.current.sortConfig.key).toBe("bank_name");
    expect(result.current.sortConfig.direction).toBe("asc");

    act(() => result.current.handleBankNameKeyDown(makeKeyEvent(" ")));
    expect(result.current.sortConfig.direction).toBe("desc");

    act(() => result.current.handleBankNameKeyDown(makeKeyEvent("Tab")));
    expect(result.current.sortConfig.direction).toBe("desc");
  });

  it("handleEditClick prefills editData.bank_name from the account catalog", () => {
    const { result } = renderHook(() =>
      useAccountTable(bankAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleEditClick(makeMouseEvent(), bankAccounts[0]));

    expect(result.current.editData?.bank_name).toBe("Fortuneo");
  });
});

describe("useAccountTable — sort by ytd_performance_pct (ACC-024, ACC-008)", () => {
  // Accounts: one positive YTD, one negative YTD, one null
  const ytdAccounts: AccountSummary[] = [
    makeAccount("a", "Alpha", "ManualMonth", 0, null, 8_000_000), // ytd=+8%, pnl=null
    makeAccount("b", "Beta", "ManualMonth", 0, null, -3_700_000), // ytd=-3.7%, pnl=null
    makeAccount("c", "Gamma", "ManualMonth", 0, null, null), // ytd=null → sorts last
  ];

  it("sorts ytd_performance_pct ascending, nulls last", () => {
    const { result } = renderHook(() =>
      useAccountTable(ytdAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("ytd_performance_pct"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
  });

  it("sorts ytd_performance_pct descending, nulls last", () => {
    const { result } = renderHook(() =>
      useAccountTable(ytdAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    act(() => result.current.handleSort("ytd_performance_pct"));
    act(() => result.current.handleSort("ytd_performance_pct"));

    expect(result.current.sortedAndFilteredAccounts.map((a) => a.id)).toEqual(["a", "b", "c"]);
  });

  it("exposes handleYtdPctKeyDown that triggers sort on Enter", () => {
    const { result } = renderHook(() =>
      useAccountTable(ytdAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    expect(result.current.sortConfig.key).toBe("name");

    act(() => result.current.handleYtdPctKeyDown(makeKeyEvent("Enter")));

    expect(result.current.sortConfig.key).toBe("ytd_performance_pct");
    expect(result.current.sortConfig.direction).toBe("asc");
  });

  it("handleYtdPctKeyDown triggers sort on Space and ignores other keys", () => {
    const { result } = renderHook(() =>
      useAccountTable(ytdAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );
    act(() => result.current.handleYtdPctKeyDown(makeKeyEvent(" ")));
    expect(result.current.sortConfig.key).toBe("ytd_performance_pct");
    expect(result.current.sortConfig.direction).toBe("asc");

    // Tab is ignored — direction stays asc (a second sort on the same key would flip it).
    act(() => result.current.handleYtdPctKeyDown(makeKeyEvent("Tab")));
    expect(result.current.sortConfig.direction).toBe("asc");
  });
});

// SYN-040 — an account carrying an inconsistent holding is marked so
// AccountTable can render the account-level marker (#account-inconsistent-{id}).
describe("useAccountTable — inconsistent holding marker (SYN-040)", () => {
  const inconsistentAccounts: AccountSummary[] = [
    makeAccount("1", "Alpha", "ManualMonth"),
    { ...makeAccount("2", "Beta", "ManualMonth"), has_inconsistent_holding: true },
  ];

  it("carries has_inconsistent_holding through into the sorted/filtered rows", () => {
    const { result } = renderHook(() =>
      useAccountTable(inconsistentAccounts, "", noopDelete, noopSummary, noopAccountClick),
    );

    const byId = Object.fromEntries(
      result.current.sortedAndFilteredAccounts.map((a) => [a.id, a.has_inconsistent_holding]),
    );
    expect(byId).toEqual({ "1": false, "2": true });
  });
});

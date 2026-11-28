import { fireEvent, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccountSummary } from "@/bindings";
import { AccountTable } from "./AccountTable";

// The delete dialogs rendered for real (AccountTable.test.tsx stubs them).
const translation = vi.hoisted(() => ({
  t: (key: string) => key,
  i18n: { language: "en" },
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => translation,
}));

const summaries: AccountSummary[] = [
  {
    holder: null,
    id: "acc-1",
    name: "Main EUR",
    currency: "EUR",
    wrapper: null,
    total_global_value: 0,
    total_unrealized_pnl: null,
    ytd_performance_pct: null,
    has_inconsistent_holding: false,
    outdated_price_count: 0,
    created_by: null,
  },
];
const summariesResult = vi.hoisted(() => ({
  holders: [] as string[],
  portfolioTotal: null,
  isLoading: false,
  error: null,
  refetch: () => Promise.resolve(),
}));
vi.mock("../useAccountSummaries", () => ({
  useAccountSummaries: () => ({ ...summariesResult, summaries }),
}));

const deleteAccountMock = vi.hoisted(() => vi.fn());
const getSummaryMock = vi.hoisted(() => vi.fn());
vi.mock("../useAccounts", () => ({
  useAccounts: () => ({
    deleteAccount: deleteAccountMock,
    getAccountDeletionSummary: getSummaryMock,
  }),
}));
vi.mock("../edit_account_modal/EditAccountModal", () => ({
  EditAccountModal: () => null,
}));
vi.mock("../useWrapperLists", () => ({
  useWrapperLists: () => null,
}));

// ACC-014 (#446) — a refused delete is told inside the dialog that asked, which stays open.
describe("AccountTable — a refused delete (ACC-014)", () => {
  beforeEach(() => {
    deleteAccountMock.mockReset();
    getSummaryMock.mockReset();
  });

  it("keeps the delete dialog open with the refusal inside it", async () => {
    getSummaryMock.mockResolvedValue({
      data: { holding_count: 0, transaction_count: 3 },
      error: null,
    });
    deleteAccountMock.mockResolvedValue({ error: { key: "error.DatabaseError" } });
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    fireEvent.click(document.querySelector("#action-delete-account-acc-1") as Element);
    await waitFor(() =>
      expect(document.querySelector("#confirm-delete-account-dialog")).not.toBeNull(),
    );
    fireEvent.click(document.querySelector("#confirm-delete-account") as Element);

    await waitFor(() =>
      expect(document.querySelector("#confirm-delete-account-error")?.textContent).toBe(
        "error.DatabaseError",
      ),
    );
    const dialog = document.querySelector("#confirm-delete-account-dialog");
    expect(dialog?.contains(document.querySelector("#confirm-delete-account-error"))).toBe(true);
    expect(deleteAccountMock).toHaveBeenCalledWith("acc-1");
  });
});

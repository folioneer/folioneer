import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset, Transaction } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { EditTransactionModal } from "./EditTransactionModal";

// CSH-018 — Cash Assets must be filtered out of the asset combobox so the user
// cannot retarget a transaction onto a system Cash Asset (Deposit/Withdrawal
// owns that flow). The filter lives in EditTransactionModal.tsx line 40.
vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({ id, items }: { id: string; items: { id: string; name: string }[] }) => (
    <div data-testid={`combobox-${id}`} data-item-ids={items.map((i) => i.id).join(",")} />
  ),
}));

vi.mock("./useEditTransactionModal", () => ({
  useEditTransactionModal: vi.fn(() => ({
    formData: {
      assetId: "asset-stock-1",
      accountId: "account-1",
      date: "2024-01-01",
      quantity: "1",
      unitPrice: "100",
      exchangeRate: "1",
      fees: "0",
      note: "",
    },
    totalAmountDisplay: "100",
    error: null,
    isSubmitting: false,
    isFormValid: true,
    showArchivedConfirm: false,
    recordPrice: false,
    setRecordPrice: vi.fn(),
    handleChange: vi.fn(),
    handleSubmit: vi.fn(),
    handleConfirmArchived: vi.fn(),
    handleCancelArchived: vi.fn(),
  })),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

const FAKE_TX: Transaction = {
  id: "tx-1",
  account_id: "account-1",
  asset_id: "asset-stock-1",
  transaction_type: "Purchase",
  date: "2024-01-01",
  quantity: 1_000_000,
  unit_price: 100_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: 100_000_000,
  note: null,
  realized_pnl: null,
  created_at: "2024-01-01T00:00:00Z",
};

describe("EditTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      assets: [
        {
          id: "asset-stock-1",
          name: "Apple",
          class: "Stocks",
          is_archived: false,
          currency: "USD",
        },
        {
          id: "system-cash-eur",
          name: "Cash EUR",
          class: "Cash",
          is_archived: false,
          currency: "EUR",
        },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account", currency: "EUR" }] as Account[],
    });
  });

  // CSH-018 — Cash Assets are filtered out of the asset combobox.
  it("filters Cash assets from the asset combobox (CSH-018)", () => {
    render(<EditTransactionModal isOpen onClose={() => {}} transaction={FAKE_TX} />);
    const combobox = screen.getByTestId("combobox-edit-trx-asset");
    const itemIds = combobox.getAttribute("data-item-ids")?.split(",") ?? [];
    expect(itemIds).toContain("asset-stock-1");
    expect(itemIds).not.toContain("system-cash-eur");
  });
});

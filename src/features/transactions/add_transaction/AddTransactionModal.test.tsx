import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { AddTransactionModal } from "./AddTransactionModal";

// CSH-018 — Cash Assets must be filtered out of the asset combobox so the user
// cannot record Buy/Sell against a system Cash Asset (Deposit/Withdrawal owns
// that flow). The filter lives in the component (line 40 of AddTransactionModal).
// Mock ComboboxField so we can inspect the items prop it receives.
vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({ id, items }: { id: string; items: { id: string; name: string }[] }) => (
    <div data-testid={`combobox-${id}`} data-item-ids={items.map((i) => i.id).join(",")} />
  ),
}));

vi.mock("./useAddTransaction", () => ({
  useAddTransaction: vi.fn(() => ({
    formData: {
      assetId: "",
      accountId: "",
      date: "",
      quantity: "",
      unitPrice: "",
      exchangeRate: "",
      fees: "",
      note: "",
    },
    totalAmountDisplay: "",
    error: null,
    isSubmitting: false,
    isFormValid: false,
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

describe("AddTransactionModal", () => {
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
        { id: "asset-bond-1", name: "Bond", class: "Bonds", is_archived: false, currency: "EUR" },
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
    render(<AddTransactionModal isOpen onClose={() => {}} prefillAccountId="account-1" />);
    const combobox = screen.getByTestId("combobox-trx-asset");
    const itemIds = combobox.getAttribute("data-item-ids")?.split(",") ?? [];
    expect(itemIds).toContain("asset-stock-1");
    expect(itemIds).toContain("asset-bond-1");
    expect(itemIds).not.toContain("system-cash-eur");
  });
});

import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { AddTransactionPage } from "./AddTransactionPage";

// CSH-018 / TRX-064 — the page offers the core's list of non-cash assets, as the dialogs
// do: a Cash Asset in the store is not offered. ComboboxField is mocked to show its items.
vi.mock("../shared/useNonCashAssets", () => ({
  useNonCashAssets: () => [
    { id: "asset-stock-1", name: "Apple", class: "Stocks", is_archived: false, currency: "USD" },
    { id: "asset-old-1", name: "OldCo", class: "Stocks", is_archived: true, currency: "EUR" },
  ],
}));

vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({ id, items }: { id: string; items: { id: string }[] }) => (
    <div data-testid={`combobox-${id}`} data-item-ids={items.map((i) => i.id).join(",")} />
  ),
}));

const form = { assetId: "", accountId: "" };
vi.mock("../add_transaction/useAddTransaction", () => ({
  useAddTransaction: vi.fn(() => ({
    formData: {
      assetId: form.assetId,
      accountId: form.accountId,
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
    handleChange: vi.fn(),
    handleSubmit: vi.fn(),
  })),
}));

const search = vi.fn();
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  useSearch: () => search(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

describe("AddTransactionPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    search.mockReturnValue({ prefillAssetId: undefined, prefillAccountId: undefined });
    form.assetId = "";
    form.accountId = "";
    useAppStore.setState({
      assets: [
        { id: "asset-stock-1", name: "Apple", class: "Stocks", is_archived: false },
        { id: "system-cash-eur", name: "Cash EUR", class: "Cash", is_archived: false },
        { id: "asset-held-1", name: "Tesla", class: "Stocks", is_archived: false, currency: "USD" },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "PEA", currency: "EUR" }] as Account[],
    });
  });

  it("offers the core's non-cash assets, never the store's Cash Asset", () => {
    render(<AddTransactionPage />);

    const offered = screen.getByTestId("combobox-trx-asset").getAttribute("data-item-ids");
    expect(offered).toBe("asset-stock-1,asset-old-1");
    expect(offered).not.toContain("system-cash-eur");
  });

  // A purchase opened on a holding names its asset at once, from the store, before the
  // core's list has arrived — here it never holds that asset — and shows the exchange rate
  // its currency calls for.
  it("names the asset it was opened on and shows its exchange rate without waiting", () => {
    search.mockReturnValue({ prefillAssetId: "asset-held-1", prefillAccountId: "account-1" });
    form.assetId = "asset-held-1";
    form.accountId = "account-1";

    render(<AddTransactionPage />);

    expect(document.getElementById("trx-asset")).toHaveValue("Tesla");
    expect(document.getElementById("trx-exchange-rate")).not.toBeNull();
  });
});

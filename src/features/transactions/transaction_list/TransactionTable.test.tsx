import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { TransactionRowViewModel } from "../shared/presenter";
import { TransactionTable } from "./TransactionTable";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "fr" } }),
}));

const row = (over: Partial<TransactionRowViewModel> = {}): TransactionRowViewModel => ({
  id: "tx-1",
  accountId: "account-1",
  assetId: "asset-1",
  assetName: "Apple",
  accountName: "My Account",
  type: "Purchase",
  date: "2024-06-14",
  quantity: "10.000",
  unitPrice: "100.000",
  exchangeRate: "1.000",
  fees: "0.000",
  totalAmount: "1000.000",
  note: null,
  realizedPnl: null,
  realizedPnlRaw: null,
  ...over,
});

const baseProps = {
  sortDirection: "desc" as const,
  onToggleSort: vi.fn(),
  onEditTransaction: vi.fn(),
  onDeleteTransaction: vi.fn(),
};

describe("TransactionTable", () => {
  it("hides the Asset column by default", () => {
    render(<TransactionTable rows={[row()]} {...baseProps} />);
    expect(screen.queryByText("transaction.column_asset")).not.toBeInTheDocument();
    expect(screen.queryByText("Apple")).not.toBeInTheDocument();
  });

  it("renders the Asset column and asset name when showAssetColumn is set", () => {
    render(<TransactionTable rows={[row()]} showAssetColumn {...baseProps} />);
    expect(screen.getByText("transaction.column_asset")).toBeInTheDocument();
    expect(screen.getByText("Apple")).toBeInTheDocument();
  });

  it("emits edit and delete intents keyed by transaction id", async () => {
    const onEditTransaction = vi.fn();
    const onDeleteTransaction = vi.fn();
    const user = userEvent.setup();
    render(
      <TransactionTable
        rows={[row({ id: "tx-9" })]}
        {...baseProps}
        onEditTransaction={onEditTransaction}
        onDeleteTransaction={onDeleteTransaction}
      />,
    );
    await user.click(screen.getByRole("button", { name: "action.edit" }));
    await user.click(screen.getByRole("button", { name: "action.delete" }));
    expect(onEditTransaction).toHaveBeenCalledWith("tx-9");
    expect(onDeleteTransaction).toHaveBeenCalledWith("tx-9");
  });

  it("renders the date in locale-numeric format", () => {
    render(<TransactionTable rows={[row({ date: "2024-06-14" })]} {...baseProps} />);
    expect(screen.getByText("14/06/2024")).toBeInTheDocument();
  });

  it("renders the money placeholder for a FreeShares row's unit price and total", () => {
    render(
      <TransactionTable
        rows={[row({ id: "fs", type: "FreeShares", unitPrice: "0.000", totalAmount: "0.000" })]}
        {...baseProps}
      />,
    );
    // unit-price and total cells fall back to the placeholder; the placeholder
    // key also renders in the (null) realized-P&L cell → 3 occurrences.
    expect(screen.getAllByText("account_details.pnl_placeholder")).toHaveLength(3);
  });

  it("renders the money placeholder for a ManagementFee row's unit price and total (FEE-055)", () => {
    render(
      <TransactionTable
        rows={[row({ id: "mf", type: "ManagementFee", unitPrice: "0.000", totalAmount: "0.000" })]}
        {...baseProps}
      />,
    );
    // unit-price and total cells fall back to the placeholder; the placeholder
    // key also renders in the (null) realized-P&L cell → 3 occurrences.
    expect(screen.getAllByText("account_details.pnl_placeholder")).toHaveLength(3);
  });

  it("renders the money placeholder for an Interest row's unit price and total (INT-030)", () => {
    render(
      <TransactionTable
        rows={[row({ id: "int", type: "Interest", unitPrice: "0.000", totalAmount: "0.000" })]}
        {...baseProps}
      />,
    );
    // unit-price and total cells fall back to the placeholder; the placeholder
    // key also renders in the (null) realized-P&L cell → 3 occurrences.
    expect(screen.getAllByText("account_details.pnl_placeholder")).toHaveLength(3);
  });

  it("renders the money placeholder for a Split row's unit price and total (SPL-060)", () => {
    render(
      <TransactionTable
        rows={[
          row({
            id: "spl",
            type: "Split",
            quantity: "×2",
            unitPrice: "0.000",
            totalAmount: "0.000",
          }),
        ]}
        {...baseProps}
      />,
    );
    // unit-price and total cells fall back to the placeholder; the placeholder
    // key also renders in the (null) realized-P&L cell → 3 occurrences.
    expect(screen.getAllByText("account_details.pnl_placeholder")).toHaveLength(3);
    // SPL-060 — the quantity column carries the ×N ratio label.
    expect(screen.getByText("×2")).toBeInTheDocument();
  });

  it("colours a positive realized P&L as a gain", () => {
    render(
      <TransactionTable
        rows={[row({ id: "pos", realizedPnl: "120.000", realizedPnlRaw: 120 * 1_000_000 })]}
        {...baseProps}
      />,
    );
    const pnl = screen.getByText("120.000");
    expect(pnl).toHaveClass("text-m3-gain");
  });

  it("colours a negative realized P&L as a loss", () => {
    render(
      <TransactionTable
        rows={[row({ id: "neg", realizedPnl: "-50.000", realizedPnlRaw: -50 * 1_000_000 })]}
        {...baseProps}
      />,
    );
    expect(screen.getByText("-50.000")).toHaveClass("text-m3-loss");
  });

  it("replaces Total Amount with Cash out/in/Balance in cashStatement mode", () => {
    render(
      <TransactionTable
        rows={[row({ cashOut: "300,000", cashIn: "", balance: "700,000" })]}
        {...baseProps}
        cashStatement
      />,
    );
    expect(screen.getByText("transaction.column_cash_out")).toBeInTheDocument();
    expect(screen.getByText("transaction.column_cash_in")).toBeInTheDocument();
    expect(screen.getByText("transaction.column_balance")).toBeInTheDocument();
    expect(screen.queryByText("transaction.column_total_amount")).not.toBeInTheDocument();
    expect(document.getElementById("txl-cash-out-tx-1")).toHaveTextContent("300,000");
    expect(document.getElementById("txl-balance-tx-1")).toHaveTextContent("700,000");
    // Empty cash-in cell falls back to the neutral placeholder.
    expect(document.getElementById("txl-cash-in-tx-1")).toHaveTextContent(
      "account_details.pnl_placeholder",
    );
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Transaction } from "@/bindings";
import { patchModalSearch } from "@/lib/modalSearch";
import { routeEditTransaction } from "./routeEditTransaction";

vi.mock("@/lib/modalSearch", () => ({ patchModalSearch: vi.fn() }));

const navigate = vi.fn();

const tx = (type: Transaction["transaction_type"]): Transaction =>
  ({
    id: "tx-1",
    account_id: "acc-1",
    asset_id: "asset-1",
    transaction_type: type,
    date: "2024-01-01",
    quantity: 0,
    unit_price: 0,
    exchange_rate: 0,
    fees: 0,
    total_amount: 0,
    note: null,
    realized_pnl: null,
    created_at: "2024-01-01T00:00:00Z",
  }) as Transaction;

describe("routeEditTransaction", () => {
  beforeEach(() => vi.clearAllMocks());

  it("routes a Deposit to the cash-deposit modal", () => {
    const openGeneric = vi.fn();
    routeEditTransaction(navigate, tx("Deposit"), openGeneric);
    expect(patchModalSearch).toHaveBeenCalledWith(navigate, {
      modal: "edit-cash-deposit",
      editTxId: "tx-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-1",
    });
    expect(openGeneric).not.toHaveBeenCalled();
  });

  it("routes a Withdrawal to the cash-withdrawal modal", () => {
    routeEditTransaction(navigate, tx("Withdrawal"), vi.fn());
    expect(patchModalSearch).toHaveBeenCalledWith(
      navigate,
      expect.objectContaining({ modal: "edit-cash-withdrawal" }),
    );
  });

  it("routes FreeShares to the free-shares modal", () => {
    routeEditTransaction(navigate, tx("FreeShares"), vi.fn());
    expect(patchModalSearch).toHaveBeenCalledWith(
      navigate,
      expect.objectContaining({ modal: "edit-free-shares" }),
    );
  });

  it("routes ManagementFee to the management-fee edit modal (FEE-063)", () => {
    routeEditTransaction(navigate, tx("ManagementFee"), vi.fn());
    expect(patchModalSearch).toHaveBeenCalledWith(
      navigate,
      expect.objectContaining({ modal: "edit-management-fee" }),
    );
  });

  it("routes a Split to the split edit modal (SPL-030)", () => {
    routeEditTransaction(navigate, tx("Split"), vi.fn());
    expect(patchModalSearch).toHaveBeenCalledWith(
      navigate,
      expect.objectContaining({ modal: "edit-split" }),
    );
  });

  it("routes Interest to the interest edit modal (INT-040)", () => {
    routeEditTransaction(navigate, tx("Interest"), vi.fn());
    expect(patchModalSearch).toHaveBeenCalledWith(
      navigate,
      expect.objectContaining({ modal: "edit-interest" }),
    );
  });

  it("opens the generic modal for a Purchase (default branch)", () => {
    const openGeneric = vi.fn();
    const purchase = tx("Purchase");
    routeEditTransaction(navigate, purchase, openGeneric);
    expect(openGeneric).toHaveBeenCalledWith(purchase);
    expect(patchModalSearch).not.toHaveBeenCalled();
  });

  it("opens the generic modal for a Sell (default branch)", () => {
    const openGeneric = vi.fn();
    routeEditTransaction(navigate, tx("Sell"), openGeneric);
    expect(openGeneric).toHaveBeenCalledTimes(1);
    expect(patchModalSearch).not.toHaveBeenCalled();
  });
});

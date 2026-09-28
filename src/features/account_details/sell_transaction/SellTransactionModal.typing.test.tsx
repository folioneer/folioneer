import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SellTransactionModal } from "./SellTransactionModal";

// L-019 — the real dialog and hook, the backend answering whenever the test says so.
const pending: Array<(value: unknown) => void> = [];
const { mockDraft, mockSnapshot } = vi.hoisted(() => ({
  mockDraft: vi.fn(),
  mockSnapshot: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    validateTransactionDraft: mockDraft,
    getHoldingSnapshotAsOf: mockSnapshot,
    recordAssetPrice: vi.fn(),
  },
}));
vi.mock("@/features/transactions/useTransactions", () => ({
  useTransactions: () => ({ sellHolding: vi.fn(), buyHolding: vi.fn() }),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn(), warn: vi.fn() } }));

/** Types a value the way the E2E helper does: native setter, then input and change events. */
function type(id: string, value: string) {
  const input = document.getElementById(id) as HTMLInputElement;
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

async function answerChecks() {
  await act(async () => {
    while (pending.length) {
      pending.shift()?.({
        status: "ok",
        data: { unit_price: 110_000_000, total_amount: 440_000_000 },
      });
    }
  });
}

describe("SellTransactionModal — typing while the draft check answers (L-019)", () => {
  beforeEach(() => {
    pending.length = 0;
    mockDraft.mockImplementation(() => new Promise((resolve) => pending.push(resolve)));
    mockSnapshot.mockResolvedValue({
      status: "ok",
      data: { quantity: 10_000_000, average_price: 1_000_000 },
    });
  });

  for (const answersBetweenFields of [false, true]) {
    it(`keeps each typed value when checks answer ${answersBetweenFields ? "between fields" : "at the end"}`, async () => {
      render(
        <SellTransactionModal
          isOpen
          onClose={() => {}}
          accountId="acc"
          accountName="Account"
          assetId="ast"
          assetName="Asset"
          assetCurrency="EUR"
          holdingQuantityMicro={10_000_000}
          onSubmitSuccess={() => {}}
        />,
      );
      await act(async () => {});

      await act(async () => type("sell-trx-date", "05/01/2019"));
      if (answersBetweenFields) await answerChecks();
      await act(async () => type("sell-trx-quantity", "4"));
      if (answersBetweenFields) await answerChecks();
      await act(async () => type("sell-trx-unit-price", "110"));
      await answerChecks();

      const value = (id: string) => (document.getElementById(id) as HTMLInputElement).value;
      expect(value("sell-trx-quantity")).toBe("4");
      expect(value("sell-trx-unit-price")).toBe("110");
      expect(value("sell-trx-date")).toBe("05/01/2019");
    });
  }
});

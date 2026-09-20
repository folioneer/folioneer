import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AddTransactionModalMount } from "./AddTransactionModalMount";

const { mockUseSearch } = vi.hoisted(() => ({ mockUseSearch: vi.fn() }));

vi.mock("@tanstack/react-router", () => ({
  useSearch: () => mockUseSearch(),
  useNavigate: () => vi.fn(),
  useRouterState: () => "/accounts/acc-1",
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));

// Stub the cross-feature modal so the mount renders in isolation; surface the prefill.
vi.mock("@/features/transactions/add_transaction/AddTransactionModal", () => ({
  AddTransactionModal: ({ prefillAccountId }: { prefillAccountId?: string }) => (
    <div data-testid="add-transaction-modal">{prefillAccountId ?? "no-prefill"}</div>
  ),
}));

describe("AddTransactionModalMount (ACD-035/036)", () => {
  beforeEach(() => mockUseSearch.mockReset());

  it("renders nothing when no add-transaction modal param is present", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<AddTransactionModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing for an unrelated modal param", () => {
    mockUseSearch.mockReturnValue({ modal: "edit-asset" });
    const { container } = render(<AddTransactionModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the Add Transaction modal with the prefilled account when active", () => {
    mockUseSearch.mockReturnValue({ modal: "add-transaction", prefillAccountId: "acc-1" });
    render(<AddTransactionModalMount />);
    expect(screen.getByTestId("add-transaction-modal")).toHaveTextContent("acc-1");
  });
});

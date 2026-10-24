import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TransferEditModalMount } from "./TransferEditModalMount";

const { mockUseSearch } = vi.hoisted(() => ({ mockUseSearch: vi.fn() }));

vi.mock("@tanstack/react-router", () => ({
  useSearch: () => mockUseSearch(),
  useNavigate: () => vi.fn(),
}));

// Stub the form so the mount renders in isolation; surface what it is given.
vi.mock("@/features/account_details/transfer_transaction/TransferEditModal", () => ({
  TransferEditModal: ({ transferId }: { transferId: string }) => (
    <div data-testid="transfer-edit">{transferId}</div>
  ),
}));

describe("TransferEditModalMount (TRF-057)", () => {
  beforeEach(() => mockUseSearch.mockReset());

  it("renders nothing without the edit-transfer params", () => {
    mockUseSearch.mockReturnValue({ modal: "edit-write-off", editTransferId: "trf-1" });
    const { container } = render(<TransferEditModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("opens the transfer named in the URL", () => {
    mockUseSearch.mockReturnValue({ modal: "edit-transfer", editTransferId: "trf-1" });
    render(<TransferEditModalMount />);
    expect(screen.getByTestId("transfer-edit")).toHaveTextContent("trf-1");
  });
});

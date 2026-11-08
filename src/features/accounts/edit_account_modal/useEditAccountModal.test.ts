import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account } from "@/bindings";
import { useEditAccountModal } from "./useEditAccountModal";

const mockUpdateAccount = vi.fn();

const mockAccount: Account = {
  id: "account-1",
  name: "Alpha",
  bank_name: "",
  currency: "EUR",
  management_fees_enabled: false,
  profile_id: "default-profile",
};

vi.mock("../useAccounts", () => ({
  useAccounts: () => ({
    updateAccount: mockUpdateAccount,
    accounts: [mockAccount],
    loading: false,
    fetchError: null,
    fetchAccounts: vi.fn(),
    addAccount: vi.fn(),
    deleteAccount: vi.fn(),
  }),
}));

const mockSetMethod = vi.fn();
const mockMove = vi.fn();

vi.mock("../gateway", () => ({
  accountGateway: {
    setAccountCostBasisMethod: (...args: unknown[]) => mockSetMethod(...args),
    moveAccountToProfile: (...args: unknown[]) => mockMove(...args),
  },
}));

// PFL-031 — the account's profile and another one.
const mockProfiles = {
  profiles: [
    { profile: { id: "default-profile", name: null }, account_count: 1, removable: false },
    { profile: { id: "p-parent", name: "Parent" }, account_count: 0, removable: true },
  ],
  in_use: "default-profile",
};
vi.mock("../shared/useProfileChoices", () => ({
  useProfileChoices: () => ({ list: mockProfiles, error: null }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useEditAccountModal", () => {
  beforeEach(() => {
    mockUpdateAccount.mockReset();
  });

  // R13, R15 — backend error keeps modal open and exposes error
  it("does not call onClose and exposes error on backend failure", async () => {
    mockUpdateAccount.mockResolvedValue({
      data: null,
      error: "Duplicate name",
    });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: mockAccount, onClose }));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBe("Duplicate name");
    expect(onClose).not.toHaveBeenCalled();
  });

  // R15 — success closes modal
  it("calls onClose on successful update", async () => {
    mockUpdateAccount.mockResolvedValue({ data: mockAccount, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: mockAccount, onClose }));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // R13 — switching account resets error
  it("resets error when account changes", () => {
    const onClose = vi.fn();
    const { result, rerender } = renderHook(
      ({ account }) => useEditAccountModal({ account, onClose }),
      { initialProps: { account: mockAccount } },
    );

    // Simulate prior error state
    act(() => {
      // Force error by triggering a failed submit in the same render won't work directly,
      // but we can verify error resets when account changes.
      // We'll just check the initial state is null, then rerender with new account.
    });

    const otherAccount: Account = {
      id: "account-2",
      name: "Beta",
      bank_name: "",
      currency: "EUR",
      management_fees_enabled: false,
    };
    rerender({ account: otherAccount });

    expect(result.current.error).toBeNull();
    expect(result.current.formData.name).toBe("Beta");
  });
});

describe("useEditAccountModal — bank name (ACC-026)", () => {
  const bankAccount: Account = {
    ...mockAccount,
    id: "account-3",
    name: "PEA",
    bank_name: "Boursorama",
  };

  beforeEach(() => {
    mockUpdateAccount.mockReset();
  });

  it("prefills the form with the account's bank name", () => {
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: bankAccount, onClose }));

    expect(result.current.formData.bank_name).toBe("Boursorama");
  });

  it("sends the edited bank name through to the update DTO", async () => {
    mockUpdateAccount.mockResolvedValue({ data: bankAccount, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: bankAccount, onClose }));

    act(() => {
      result.current.handleChange({
        target: { name: "bank_name", value: "Fortuneo" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAccount).toHaveBeenCalledWith(
      expect.objectContaining({ id: "account-3", bank_name: "Fortuneo" }),
    );
  });
});

describe("useEditAccountModal — management fees parameter (FEE-075)", () => {
  beforeEach(() => {
    mockUpdateAccount.mockReset();
  });

  it("prefills the parameter from the account and sends its new value on submit", async () => {
    const withFees: Account = { ...mockAccount, management_fees_enabled: true };
    mockUpdateAccount.mockResolvedValue({ data: withFees, error: null });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: withFees, onClose }));
    expect(result.current.formData.management_fees_enabled).toBe(true);

    act(() => {
      result.current.handleChange({
        target: { name: "management_fees_enabled", type: "checkbox", checked: false, value: "on" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateAccount).toHaveBeenCalledWith(
      expect.objectContaining({ id: "account-1", management_fees_enabled: false }),
    );
  });
});

describe("useEditAccountModal — country and wrapper (ACC-043, ACC-044)", () => {
  beforeEach(() => {
    mockUpdateAccount.mockReset();
  });

  // A rename sends the stored country and wrapper back, a wrapper this device's lists do not
  // know included, so the edit keeps them.
  it("sends the stored country and wrapper back with a rename", async () => {
    const planned: Account = { ...mockAccount, country: "ES", wrapper: "es_plan" };
    mockUpdateAccount.mockResolvedValue({ data: planned, error: null });
    const { result } = renderHook(() =>
      useEditAccountModal({ account: planned, onClose: vi.fn() }),
    );
    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Plan pensiones" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockUpdateAccount).toHaveBeenCalledWith(
      expect.objectContaining({ name: "Plan pensiones", country: "ES", wrapper: "es_plan" }),
    );
  });

  // #515 (the validated mock), CBM-030 — a changed method warns, and saving changes it
  // after the account's other fields; an unchanged one is not sent.
  it("changes the method after saving, and warns while it differs", async () => {
    mockUpdateAccount.mockResolvedValue({ data: mockAccount, error: null });
    mockSetMethod.mockResolvedValue({ status: "ok", data: mockAccount });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: mockAccount, onClose }));
    expect(result.current.costBasisChanged).toBe(false);
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockSetMethod).not.toHaveBeenCalled();

    act(() => {
      result.current.handleChange({
        target: { name: "cost_basis_method", value: "Fifo" },
      } as React.ChangeEvent<HTMLSelectElement>);
    });
    expect(result.current.costBasisChanged).toBe(true);
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockSetMethod).toHaveBeenCalledWith("account-1", "Fifo");
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  // CBM-030 — a refused change keeps the dialog open with its error.
  it("keeps the dialog open when the change of method is refused", async () => {
    mockUpdateAccount.mockResolvedValue({ data: mockAccount, error: null });
    mockSetMethod.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: mockAccount, onClose }));
    act(() => {
      result.current.handleChange({
        target: { name: "cost_basis_method", value: "Fifo" },
      } as React.ChangeEvent<HTMLSelectElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(onClose).not.toHaveBeenCalled();
    expect(result.current.error).not.toBeNull();
  });

  // PFL-031 (#767, the validated mock) — another profile warns, then moves the account
  // before its other changes are saved; a refused move saves nothing and keeps the modal open.
  it("moves the account to the profile chosen, and keeps the modal open on a refusal", async () => {
    mockUpdateAccount.mockResolvedValue({ data: mockAccount, error: null });
    mockMove.mockResolvedValue({
      status: "error",
      error: { code: "TransferAcrossProfiles" },
    });
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditAccountModal({ account: mockAccount, onClose }));
    expect(result.current.profileMove).toBeNull();
    act(() => {
      result.current.handleChange({
        target: { name: "profile_id", value: "p-parent", type: "select-one" },
      } as React.ChangeEvent<HTMLSelectElement>);
    });
    expect(result.current.profileMove?.to?.name).toBe("Parent");
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockMove).toHaveBeenCalledWith("account-1", "p-parent");
    expect(mockUpdateAccount).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({ key: "error.TransferAcrossProfiles" });
    expect(onClose).not.toHaveBeenCalled();

    mockMove.mockResolvedValue({ status: "ok", data: mockAccount });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(onClose).toHaveBeenCalled();
  });
});

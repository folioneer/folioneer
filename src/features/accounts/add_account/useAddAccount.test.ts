import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { useAddAccount } from "./useAddAccount";

const mockAddAccount = vi.fn();

vi.mock("../useAccounts", () => ({
  useAccounts: () => ({
    addAccount: mockAddAccount,
    accounts: [],
    loading: false,
    fetchError: null,
    fetchAccounts: vi.fn(),
    updateAccount: vi.fn(),
    deleteAccount: vi.fn(),
  }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useAddAccount", () => {
  beforeEach(() => {
    mockAddAccount.mockReset();
  });

  // R14 — empty name blocks submission
  it("sets error and does not call addAccount when name is empty", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useAddAccount({ onSubmitSuccess }));

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).not.toBeNull();
    expect(mockAddAccount).not.toHaveBeenCalled();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // R14 — whitespace-only name blocks submission
  it("sets error and does not call addAccount when name is whitespace only", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useAddAccount({ onSubmitSuccess }));

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "   " },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).not.toBeNull();
    expect(mockAddAccount).not.toHaveBeenCalled();
  });

  // R13 — backend error keeps modal open and exposes error
  it("sets error and does not call onSubmitSuccess on backend error", async () => {
    mockAddAccount.mockResolvedValue({ data: null, error: "Duplicate name" });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useAddAccount({ onSubmitSuccess }));

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Alpha" },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBe("Duplicate name");
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // R13 — success calls onSubmitSuccess and clears error
  it("calls onSubmitSuccess and clears error on success", async () => {
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useAddAccount({ onSubmitSuccess }));

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Alpha" },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });
});

describe("useAddAccount — country and wrapper (ACC-044)", () => {
  beforeEach(() => {
    mockAddAccount.mockReset();
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
  });

  const change = (
    result: { current: ReturnType<typeof useAddAccount> },
    name: string,
    value: string,
  ) =>
    act(() => {
      result.current.handleChange({
        target: { name, value },
      } as React.ChangeEvent<HTMLInputElement>);
    });

  it("sends the chosen wrapper, and none for the portfolio's country", async () => {
    const { result } = renderHook(() => useAddAccount());
    change(result, "name", "PEA");
    change(result, "wrapper", "fr_pea");
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockAddAccount).toHaveBeenCalledWith(
      expect.objectContaining({ country: null, wrapper: "fr_pea" }),
    );
  });

  // ACC-043 — any account may hold any wrapper: a change of country never empties it.
  it("keeps the wrapper when the country changes", () => {
    const { result } = renderHook(() => useAddAccount());
    change(result, "wrapper", "fr_pea");
    act(() => result.current.setCountry("DE"));
    expect(result.current.formData.country).toBe("DE");
    expect(result.current.formData.wrapper).toBe("fr_pea");
  });
});

describe("useAddAccount — bank name (ACC-026)", () => {
  beforeEach(() => {
    mockAddAccount.mockReset();
  });

  it("passes the bank name field through to the create DTO", async () => {
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
    const { result } = renderHook(() => useAddAccount());

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "PEA" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    act(() => {
      result.current.handleChange({
        target: { name: "bank_name", value: "Boursorama" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAccount).toHaveBeenCalledWith(
      expect.objectContaining({ bank_name: "Boursorama" }),
    );
  });

  it("defaults the bank name to empty string when untouched and resets it after success", async () => {
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
    const { result } = renderHook(() => useAddAccount());

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Plain" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAccount).toHaveBeenCalledWith(expect.objectContaining({ bank_name: "" }));
    expect(result.current.formData.bank_name).toBe("");
  });
});

describe("useAddAccount — management fees opt-in (FEE-075)", () => {
  beforeEach(() => {
    mockAddAccount.mockReset();
  });

  it("passes the checkbox state through to the create DTO", async () => {
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
    const { result } = renderHook(() => useAddAccount());

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Assurance Vie" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    act(() => {
      result.current.handleChange({
        target: { name: "management_fees_enabled", type: "checkbox", checked: true, value: "on" },
      } as unknown as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAccount).toHaveBeenCalledWith(
      expect.objectContaining({ management_fees_enabled: true }),
    );
  });

  it("defaults the flag to false when untouched", async () => {
    mockAddAccount.mockResolvedValue({ data: { id: "new-id" }, error: null });
    const { result } = renderHook(() => useAddAccount());

    act(() => {
      result.current.handleChange({
        target: { name: "name", value: "Plain" },
      } as React.ChangeEvent<HTMLInputElement>);
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockAddAccount).toHaveBeenCalledWith(
      expect.objectContaining({ management_fees_enabled: false }),
    );
  });
});

describe("useAddAccount — reference currency (FXR-128)", () => {
  it("starts a new account in the reference currency", () => {
    useAppStore.setState({ referenceCurrency: "CHF" });
    const { result } = renderHook(() => useAddAccount());
    expect(result.current.formData.currency).toBe("CHF");
    useAppStore.setState({ referenceCurrency: "EUR" });
  });
});

import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account } from "@/bindings";
import { useAppStore } from "@/lib/store";

const { mockNavigate } = vi.hoisted(() => ({ mockNavigate: vi.fn() }));

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mockNavigate,
  useRouterState: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { useRouterState } from "@tanstack/react-router";

const mockRouterState = vi.mocked(useRouterState);

const { useHeaderConfig } = await import("./useHeaderConfig");

function makeLocation(pathname: string, searchStr = "") {
  mockRouterState.mockReturnValue({
    pathname,
    searchStr,
  } as unknown as ReturnType<typeof useRouterState>);
}

describe("useHeaderConfig", () => {
  beforeEach(() => {
    useAppStore.setState({
      accounts: [{ id: "acc-1", name: "Savings" }] as Account[],
    });
  });

  // /accounts/$accountId/transactions/$assetId
  it("returns transaction list title with back to account details for tx list route", () => {
    makeLocation("/accounts/acc-1/transactions/asset-1");
    const { result } = renderHook(() => useHeaderConfig());
    expect(result.current.title).toBe("transaction.list_title");
    expect(result.current.onBack).toBeDefined();
  });

  // /accounts/$accountId
  it("returns account name for account detail route", () => {
    makeLocation("/accounts/acc-1");
    const { result } = renderHook(() => useHeaderConfig());
    expect(result.current.title).toBe("Savings");
    expect(result.current.onBack).toBeDefined();
  });

  it("returns fallback title for unknown account id", () => {
    makeLocation("/accounts/unknown-id");
    const { result } = renderHook(() => useHeaderConfig());
    expect(result.current.title).toBe("account_details.title");
  });

  // /accounts (top-level nav item)
  it("returns nav label for top-level accounts route", () => {
    makeLocation("/accounts");
    const { result } = renderHook(() => useHeaderConfig());
    expect(result.current.title).toBe("nav.accounts");
    expect(result.current.onBack).toBeUndefined();
  });

  // unknown path
  it("returns empty string for unknown route", () => {
    makeLocation("/unknown/path");
    const { result } = renderHook(() => useHeaderConfig());
    expect(result.current.title).toBe("");
  });
});

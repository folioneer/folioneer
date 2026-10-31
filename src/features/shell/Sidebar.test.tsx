import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  useRouterState: () => "/accounts",
}));
vi.mock("@/features/about", () => ({
  AboutModal: ({ isOpen }: { isOpen: boolean }) =>
    isOpen ? <div data-testid="about-modal" /> : null,
}));
vi.mock("@/features/settings", () => ({
  SettingsDialog: ({ isOpen }: { isOpen: boolean }) =>
    isOpen ? <div data-testid="settings-dialog" /> : null,
}));
vi.mock("./SidebarVersion", () => ({ SidebarVersion: () => null }));

describe("Sidebar", () => {
  // The header stacks over anything inside the drawer.
  it("opens Settings over the screen, outside the drawer", () => {
    render(<Sidebar isOpen toggleDrawer={vi.fn()} />);

    expect(screen.queryByTestId("settings-dialog")).toBeNull();
    fireEvent.click(document.getElementById("nav-settings") as HTMLElement);

    expect(screen.getByTestId("settings-dialog").closest("aside")).toBeNull();
  });
});

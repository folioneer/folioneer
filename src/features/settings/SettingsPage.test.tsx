import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

vi.mock("./useSettings", () => ({
  useSettings: () => ({
    currentChoice: "auto",
    setLanguage: vi.fn(),
    autoRecordPrice: false,
    toggleAutoRecordPrice: vi.fn(),
    autoFetch: false,
    toggleAutoFetch: vi.fn(),
  }),
}));

// Stub the scheduled-fetch section so this test exercises SettingsPage's own
// JSX wiring only (no hook/gateway concerns — covered by
// ScheduledFetchSection.test.tsx and useScheduledFetchSection.test.ts).
vi.mock("./scheduled_fetch/ScheduledFetchSection", () => ({
  ScheduledFetchSection: () => <div data-testid="scheduled-fetch-section-mounted" />,
}));

const { SettingsPage } = await import("./SettingsPage");
const { useAppStore } = await import("@/lib/store");

// [unit-test-needed] SettingsPage.tsx:SettingsPage — SPF-010 mounts the new
// "Daily price download" section alongside the existing settings.
describe("SettingsPage — scheduled fetch section (SPF-010)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("mounts the ScheduledFetchSection alongside the existing settings", () => {
    render(<SettingsPage />);

    expect(screen.getByTestId("scheduled-fetch-section-mounted")).toBeInTheDocument();
  });
});

// #035 — the shell's content area never scrolls, so the page must scroll itself: in a
// window shorter than the page, every setting down to the last one stays reachable.
describe("SettingsPage — scrolling (#035)", () => {
  it("is its own vertical scroll container, holding every setting down to the last", () => {
    const { container } = render(<SettingsPage />);

    const page = container.firstElementChild;
    expect(page).toHaveClass("min-h-0", "flex-1", "overflow-y-auto");
    expect(page).toContainElement(container.querySelector("#settings-auto-record-price"));
  });
});

describe("SettingsPage — without an External provider (MKT-212, SPF-071)", () => {
  afterEach(() => {
    useAppStore.setState({ hasExternalProvider: true });
  });

  it("shows the auto-fetch setting and the scheduled fetch with an External provider", () => {
    const { container } = render(<SettingsPage />);
    expect(container.querySelector("#settings-auto-fetch")).toBeInTheDocument();
    expect(screen.getByTestId("scheduled-fetch-section-mounted")).toBeInTheDocument();
  });

  it("shows neither in a build without one, and keeps every other setting", () => {
    useAppStore.setState({ hasExternalProvider: false });
    const { container } = render(<SettingsPage />);

    expect(container.querySelector("#settings-auto-fetch")).toBeNull();
    expect(screen.queryByTestId("scheduled-fetch-section-mounted")).toBeNull();
    expect(container.querySelector("#settings-auto-record-price")).toBeInTheDocument();
  });
});

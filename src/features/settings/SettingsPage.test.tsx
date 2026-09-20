import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

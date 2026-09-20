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

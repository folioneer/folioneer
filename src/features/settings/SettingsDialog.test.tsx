import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const setLanguage = vi.fn();
const toggleAutoRecordPrice = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

vi.mock("./useSettings", () => ({
  useSettings: () => ({
    currentChoice: "auto",
    setLanguage,
    autoRecordPrice: false,
    toggleAutoRecordPrice,
    autoFetch: false,
    toggleAutoFetch: vi.fn(),
  }),
}));

// Each of these reads its own state and has its own tests.
vi.mock("./scheduled_fetch/ScheduledFetchSection", () => ({
  ScheduledFetchSection: () => <div data-testid="scheduled-fetch-section-mounted" />,
}));
vi.mock("./reference_currency/ReferenceCurrencySection", () => ({
  ReferenceCurrencySection: () => <div data-testid="reference-currency-section-mounted" />,
}));
vi.mock("./sync/SyncSection", () => ({
  SyncSection: () => <div data-testid="sync-section-mounted" />,
}));
vi.mock("./agent_connection/AgentsAllowedSetting", () => ({
  AgentsAllowedSetting: () => <div data-testid="agents-allowed-mounted" />,
}));

const { SettingsDialog } = await import("./SettingsDialog");
const { useAppStore } = await import("@/lib/store");

const element = (id: string) => document.getElementById(id);
const open = (id: string) => fireEvent.click(element(id) as HTMLElement);

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    useAppStore.setState({ hasExternalProvider: true });
  });

  it("shows nothing while closed", () => {
    render(<SettingsDialog isOpen={false} onClose={vi.fn()} />);

    expect(element("settings-dialog")).toBeNull();
  });

  it("opens on General: the language, the reference currency and the auto-record", () => {
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    expect(element("settings-section-general")).toHaveAttribute("aria-current", "true");
    expect(element("settings-language")).toBeInTheDocument();
    expect(screen.getByTestId("reference-currency-section-mounted")).toBeInTheDocument();
    expect(element("settings-auto-record-price")).toBeInTheDocument();
    expect(element("settings-auto-fetch")).toBeNull();
  });

  it("the language is a list, and choosing in it sets the language", () => {
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    fireEvent.change(element("settings-language") as HTMLElement, { target: { value: "fr" } });

    expect(setLanguage).toHaveBeenCalledWith("fr");
  });

  // MKT-050 — the toggle in General.
  it("switching the auto-record calls its setting", () => {
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    open("settings-auto-record-price");

    expect(toggleAutoRecordPrice).toHaveBeenCalledTimes(1);
  });

  // SPF-010, MKT-120 — the fetch settings sit in Prices.
  it("shows one section at a time", () => {
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    open("settings-section-prices");
    expect(element("settings-section-prices")).toHaveAttribute("aria-current", "true");
    expect(element("settings-section-general")).not.toHaveAttribute("aria-current");
    expect(element("settings-auto-fetch")).toBeInTheDocument();
    expect(screen.getByTestId("scheduled-fetch-section-mounted")).toBeInTheDocument();
    expect(element("settings-language")).toBeNull();

    open("settings-section-agents");
    expect(screen.getByTestId("agents-allowed-mounted")).toBeInTheDocument();
    expect(element("settings-auto-fetch")).toBeNull();
  });

  // MKT-212, SPF-071 — with no External provider there is nothing to show in Prices.
  it("has no Prices section with no External provider, and keeps the others", () => {
    useAppStore.setState({ hasExternalProvider: false });
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    expect(element("settings-section-prices")).toBeNull();
    expect(element("settings-section-general")).toBeInTheDocument();
    expect(element("settings-section-sync")).toBeInTheDocument();
    expect(element("settings-section-agents")).toBeInTheDocument();
  });

  it("holds the whole of sync in its Sync section", () => {
    render(<SettingsDialog isOpen onClose={vi.fn()} />);

    open("settings-section-sync");

    expect(screen.getByTestId("sync-section-mounted")).toBeInTheDocument();
  });

  it("closes from its button", () => {
    const onClose = vi.fn();
    render(<SettingsDialog isOpen onClose={onClose} />);

    open("settings-close");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("reopens on General", () => {
    const { rerender } = render(<SettingsDialog isOpen onClose={vi.fn()} />);
    open("settings-section-agents");

    rerender(<SettingsDialog isOpen={false} onClose={vi.fn()} />);
    rerender(<SettingsDialog isOpen onClose={vi.fn()} />);

    expect(element("settings-section-general")).toHaveAttribute("aria-current", "true");
  });
});

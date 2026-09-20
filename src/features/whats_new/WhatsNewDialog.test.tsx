import { configure, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ChangelogSection } from "./parseChangelog";

// Resolve getByTestId against `id` so assertions target the same stable
// selectors the E2E suite uses (F25).
configure({ testIdAttribute: "id" });

// Mock react-i18next — return the key so assertions use i18n keys (F24).
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en-US" },
  }),
}));

import { WhatsNewDialog } from "./WhatsNewDialog";

const SECTIONS: ChangelogSection[] = [
  {
    version: "0.34.0",
    date: "2026-07-05",
    body: ["### Added", "- newest feature", "- second bullet"],
  },
  {
    version: "0.33.0",
    date: "2026-07-04",
    body: ["### Fixed", "- a fix"],
  },
];

describe("WhatsNewDialog", () => {
  it("renders the stacked sections with version, date, subheads, and bullets", () => {
    render(<WhatsNewDialog version="0.34.0" sections={SECTIONS} onDismiss={vi.fn()} />);

    expect(screen.getByTestId("whats-new-dialog")).toBeTruthy();
    expect(screen.getByText('whats_new.title:{"version":"0.34.0"}')).toBeTruthy();
    expect(screen.getByTestId("whats-new-section-0.34.0").textContent).toContain("2026-07-05");
    expect(screen.getByText("Added")).toBeTruthy();
    expect(screen.getByText("newest feature")).toBeTruthy();
    expect(screen.getByText("second bullet")).toBeTruthy();
    expect(screen.getByTestId("whats-new-section-0.33.0").textContent).toContain("a fix");
  });

  it("dismisses through the single action button", async () => {
    const onDismiss = vi.fn();
    render(<WhatsNewDialog version="0.34.0" sections={SECTIONS} onDismiss={onDismiss} />);

    await userEvent.click(screen.getByTestId("whats-new-dismiss"));

    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});

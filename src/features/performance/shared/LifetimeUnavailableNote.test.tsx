import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LifetimeUnavailableNote } from "./LifetimeUnavailableNote";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, string>) => `${key}:${JSON.stringify(vars ?? {})}`,
  }),
}));

describe("LifetimeUnavailableNote", () => {
  // PRF-088 — the note shows the message it is given, above the table.
  it("shows the note it is given", () => {
    render(
      <LifetimeUnavailableNote
        note={{ key: "account_performance.lifetime_unavailable_no_invested_capital" }}
        idPrefix="account-performance"
      />,
    );
    const note = screen.getByRole("note");
    expect(note.id).toBe("account-performance-lifetime-note");
    expect(note.textContent).toContain("lifetime_unavailable_no_invested_capital");
  });

  // PRF-088 — nothing renders while the lifetime metrics are present.
  it("renders nothing without a note", () => {
    const { container } = render(<LifetimeUnavailableNote note={null} idPrefix="x" />);
    expect(container.firstChild).toBeNull();
  });
});

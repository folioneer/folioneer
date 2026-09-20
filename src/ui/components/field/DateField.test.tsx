import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { DateField } from "./DateField";

// i18n mock: t() echoes the key; language is French so the display format is DD/MM/YYYY.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "fr" },
  }),
}));

const pad2 = (n: number) => String(n).padStart(2, "0");
const todayIso = () => {
  const d = new Date();
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
};

/** Controlled harness mirroring real usage: parent stores the emitted ISO string. */
function Harness({ initial = "" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <DateField id="d" label="Date" value={value} onChange={(e) => setValue(e.target.value)} />
      <span data-testid="iso">{value}</span>
    </>
  );
}

describe("DateField", () => {
  // Regression: the controlled value prop must not wipe in-progress typing.
  it("keeps a leading zero while typing instead of collapsing 05 to 5", async () => {
    const user = userEvent.setup();
    render(<Harness initial="2026-06-20" />);
    const input = screen.getByLabelText("Date");

    await user.clear(input);
    await user.type(input, "05");

    expect(input).toHaveValue("05");
  });

  it("parses a complete locale date to ISO and preserves the typed display", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const input = screen.getByLabelText("Date");

    await user.type(input, "05/06/2026");

    expect(input).toHaveValue("05/06/2026");
    expect(screen.getByTestId("iso")).toHaveTextContent("2026-06-05");
  });

  it("does not wipe the field on a partial (incomplete) edit", async () => {
    const user = userEvent.setup();
    render(<Harness initial="2026-06-20" />);
    const input = screen.getByLabelText("Date");

    await user.clear(input);
    await user.type(input, "05/06");

    expect(input).toHaveValue("05/06");
    expect(screen.getByTestId("iso")).toBeEmptyDOMElement();
  });

  // Regression (techdebt 2026-06-21): a half-typed entry left stale text after focus
  // left, because its "" emit is indistinguishable from an external reset to "".
  it("reverts a half-typed entry to the committed value on blur", async () => {
    const user = userEvent.setup();
    render(<Harness initial="2026-06-20" />);
    const input = screen.getByLabelText("Date");

    await user.clear(input);
    await user.type(input, "05/0"); // partial → emits "", display lingers as "05/0"
    expect(input).toHaveValue("05/0");

    fireEvent.blur(input);

    // Display re-syncs to the committed value ("" here) — no stale partial text.
    expect(input).toHaveValue("");
    expect(screen.getByTestId("iso")).toBeEmptyDOMElement();
  });

  it("renders a prefilled ISO value in locale (fr) format", () => {
    render(<Harness initial="2026-06-20" />);
    expect(screen.getByLabelText("Date")).toHaveValue("20/06/2026");
  });

  it("disables browser autocomplete so it cannot collide with the calendar popup", () => {
    render(<Harness />);
    expect(screen.getByLabelText("Date")).toHaveAttribute("autocomplete", "off");
  });

  it("commits an ISO date when a calendar day is selected", async () => {
    const user = userEvent.setup();
    render(<Harness initial="2026-03-10" />);
    const input = screen.getByLabelText("Date");

    await user.click(input); // focus opens the calendar at the prefilled month
    const calendar = await screen.findByRole("dialog");
    await user.click(within(calendar).getByRole("button", { name: "15" }));

    expect(screen.getByTestId("iso")).toHaveTextContent("2026-03-15");
  });

  it("increments the date by one day on '+'", () => {
    render(<Harness initial="2026-06-20" />);
    const input = screen.getByLabelText("Date");

    fireEvent.keyDown(input, { key: "+" });

    expect(input).toHaveValue("21/06/2026");
    expect(screen.getByTestId("iso")).toHaveTextContent("2026-06-21");
  });

  it("decrements the date by one day on '-'", () => {
    render(<Harness initial="2026-06-20" />);
    const input = screen.getByLabelText("Date");

    fireEvent.keyDown(input, { key: "-" });

    expect(input).toHaveValue("19/06/2026");
    expect(screen.getByTestId("iso")).toHaveTextContent("2026-06-19");
  });

  it("jumps an empty field to today on '+'", () => {
    render(<Harness />);
    const input = screen.getByLabelText("Date");

    fireEvent.keyDown(input, { key: "+" });

    expect(screen.getByTestId("iso")).toHaveTextContent(todayIso());
  });

  it("does not hijack '-' typed mid-entry (incomplete value)", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const input = screen.getByLabelText("Date");

    await user.type(input, "05/06");
    fireEvent.keyDown(input, { key: "-" });

    // Still the partial entry — stepping was suppressed, no committed date.
    expect(input).toHaveValue("05/06");
    expect(screen.getByTestId("iso")).toBeEmptyDOMElement();
  });
});

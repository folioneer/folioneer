import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setDisplayLocale } from "@/lib/microUnits";
import { CalcField } from "./CalcField";

/** Controlled wrapper mirroring how a form hook owns the field value. */
function Harness({ initial = "", onValue }: { initial?: string; onValue?: (v: string) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <CalcField
        id="calc"
        label="Amount"
        value={value}
        onValueChange={(v) => {
          setValue(v);
          onValue?.(v);
        }}
      />
      <button type="button" onClick={() => setValue("")}>
        reset
      </button>
    </>
  );
}

describe("CalcField", () => {
  // The cases below are written with a dot: the display language is English.
  beforeEach(() => setDisplayLocale("en"));
  afterEach(() => setDisplayLocale("fr"));

  it("renders the label and the initial value", () => {
    render(<Harness initial="1.000000" />);
    expect(screen.getByLabelText("Amount")).toHaveValue("1.000000");
  });

  it("passes a plain number through unchanged and shows no hint", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    fireEvent.change(screen.getByLabelText("Amount"), { target: { value: "120.50" } });
    expect(onValue).toHaveBeenLastCalledWith("120.50");
    expect(screen.queryByText(/^=/)).not.toBeInTheDocument();
  });

  it("shows a live result hint and reports the evaluated value for an expression", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    fireEvent.change(screen.getByLabelText("Amount"), { target: { value: "100*1.2" } });
    expect(screen.getByText("= 120")).toBeInTheDocument();
    expect(onValue).toHaveBeenLastCalledWith("120");
  });

  it("commits the result into the field on blur", () => {
    render(<Harness />);
    const input = screen.getByLabelText("Amount");
    fireEvent.change(input, { target: { value: "(100+5)*1.2" } });
    expect(input).toHaveValue("(100+5)*1.2");
    fireEvent.blur(input);
    expect(input).toHaveValue("126");
  });

  it("reports the raw text and shows no hint for an incomplete expression", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    fireEvent.change(screen.getByLabelText("Amount"), { target: { value: "100*" } });
    expect(onValue).toHaveBeenLastCalledWith("100*");
    expect(screen.queryByText(/^=/)).not.toBeInTheDocument();
  });

  it("re-syncs the display when the form resets the value externally", () => {
    render(<Harness initial="50" />);
    const input = screen.getByLabelText("Amount");
    fireEvent.change(input, { target: { value: "10+5" } });
    expect(screen.getByText("= 15")).toBeInTheDocument();
    fireEvent.click(screen.getByText("reset"));
    expect(input).toHaveValue("");
  });

  it("renders an inline error message", () => {
    render(
      <CalcField id="calc" label="Amount" value="" onValueChange={vi.fn()} error="Required" />,
    );
    expect(screen.getByText("Required")).toBeInTheDocument();
  });

  // NUM-010/011 — in English only the dot is a decimal separator: a comma is left as
  // typed and handed to the form as it is, which the form's check refuses.
  it("in English, shows the dot and reads a comma as no number", () => {
    const onValue = vi.fn();
    render(<Harness initial="12.345678" onValue={onValue} />);
    const input = screen.getByLabelText("Amount");
    expect(input).toHaveValue("12.345678");
    fireEvent.change(input, { target: { value: "1,5" } });
    expect(input).toHaveValue("1,5");
    expect(onValue).toHaveBeenLastCalledWith("1,5");
  });
});

// NUM-010/011 — in French the field shows a comma and reads a comma or a dot; the form
// always receives the figure written with a dot.
describe("CalcField — in French", () => {
  beforeEach(() => setDisplayLocale("fr"));

  it("shows a recorded figure with a comma", () => {
    render(<Harness initial="12.345678" />);
    expect(screen.getByLabelText("Amount")).toHaveValue("12,345678");
  });

  it("reads a comma, and hands the form a dot", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    const input = screen.getByLabelText("Amount");
    fireEvent.change(input, { target: { value: "120,50" } });
    expect(input).toHaveValue("120,50");
    expect(onValue).toHaveBeenLastCalledWith("120.50");
  });

  it("reads a dot typed on the keypad, and shows a comma", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    const input = screen.getByLabelText("Amount");
    fireEvent.change(input, { target: { value: "120.50" } });
    expect(input).toHaveValue("120,50");
    expect(onValue).toHaveBeenLastCalledWith("120.50");
  });

  // A typed dot becomes a comma at the keystroke: the caret stays where it was.
  it("keeps the caret in place when a dot typed mid-text becomes a comma", () => {
    render(<Harness initial="125" />);
    const input = screen.getByLabelText("Amount") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "1.25", selectionStart: 2, selectionEnd: 2 } });
    expect(input).toHaveValue("1,25");
    expect(input.selectionStart).toBe(2);
  });

  // NUM-002 — a change of language rewrites what the field shows from the form's value.
  it("follows a change of language", async () => {
    const { default: i18n } = await import("@/i18n/config");
    await act(async () => {
      await i18n.changeLanguage("fr");
    });
    render(<Harness initial="1.5" />);
    const input = screen.getByLabelText("Amount");
    expect(input).toHaveValue("1,5");
    await act(async () => {
      await i18n.changeLanguage("en");
    });
    expect(input).toHaveValue("1.5");
    await act(async () => {
      await i18n.changeLanguage("fr");
    });
    expect(input).toHaveValue("1,5");
  });

  it("computes an expression typed with commas and shows its result with one", () => {
    const onValue = vi.fn();
    render(<Harness onValue={onValue} />);
    const input = screen.getByLabelText("Amount");
    fireEvent.change(input, { target: { value: "100*1,25+0,5" } });
    expect(screen.getByText("= 125,5")).toBeInTheDocument();
    expect(onValue).toHaveBeenLastCalledWith("125.5");
    fireEvent.blur(input);
    expect(input).toHaveValue("125,5");
  });
});

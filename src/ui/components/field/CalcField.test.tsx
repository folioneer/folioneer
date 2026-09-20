import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
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
});

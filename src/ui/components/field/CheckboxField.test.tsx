import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CheckboxField } from "./CheckboxField";

describe("CheckboxField", () => {
  it("shows its label and description and reports the new state", () => {
    const onChange = vi.fn();
    render(
      <CheckboxField
        id="allow"
        label="Allow"
        description="What it does"
        checked={false}
        onChange={onChange}
      />,
    );
    const box = screen.getByLabelText(/Allow/);
    expect(box).toHaveAttribute("id", "allow");
    expect(box).toHaveAccessibleDescription("What it does");

    fireEvent.click(box);

    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("cannot be switched while disabled", () => {
    render(<CheckboxField id="allow" label="Allow" checked onChange={vi.fn()} disabled />);
    expect(screen.getByLabelText("Allow")).toBeDisabled();
    expect(screen.getByLabelText("Allow")).toBeChecked();
  });
});

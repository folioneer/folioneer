import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FormProblem } from "./FormProblem";

describe("FormProblem", () => {
  // An error is an alert; it takes the place of a hint.
  it("shows the error as an alert, in place of the hint", () => {
    render(<FormProblem idPrefix="form" error="Not enough cash." hint="Enter the quantity." />);
    const error = document.getElementById("form-error");
    expect(error?.textContent).toBe("Not enough cash.");
    expect(error?.getAttribute("role")).toBe("alert");
    expect(document.getElementById("form-hint")).toBeNull();
  });

  // A hint alone is plain text, announced as a status, not as an alert.
  it("shows the hint plainly when there is no error", () => {
    render(<FormProblem idPrefix="form" hint="Enter the quantity." />);
    const hint = document.getElementById("form-hint");
    expect(hint?.textContent).toBe("Enter the quantity.");
    expect(hint?.getAttribute("role")).toBe("status");
  });

  // Nothing to say, nothing rendered.
  it("renders nothing without an error or a hint", () => {
    const { container } = render(<FormProblem idPrefix="form" />);
    expect(container.firstChild).toBeNull();
  });
});

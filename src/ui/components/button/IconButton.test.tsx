import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { IconButton } from "./IconButton";

// #712 (the validated mock) — an icon button that asks for it shows its name on hover and on
// keyboard focus, the name screen readers already read; one that does not, shows nothing.
describe("IconButton — its name as a label (#712)", () => {
  it("shows its name while hovered, and hides it when the pointer leaves", () => {
    render(<IconButton icon={<span />} aria-label="Buy" tooltip />);
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
    fireEvent.mouseEnter(screen.getByRole("button", { name: "Buy" }));
    expect(screen.getByTestId("icon-button-label")).toHaveTextContent("Buy");
    fireEvent.mouseLeave(screen.getByRole("button", { name: "Buy" }));
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
  });

  it("shows its name while focused, and hides it on blur", () => {
    render(<IconButton icon={<span />} aria-label="Sell" tooltip />);
    fireEvent.focus(screen.getByRole("button", { name: "Sell" }));
    expect(screen.getByTestId("icon-button-label")).toHaveTextContent("Sell");
    fireEvent.blur(screen.getByRole("button", { name: "Sell" }));
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
  });

  it("shows no label unless asked", () => {
    render(<IconButton icon={<span />} aria-label="Note" />);
    fireEvent.mouseEnter(screen.getByRole("button", { name: "Note" }));
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
  });

  it("closes on Escape and on a scroll, and is hidden from assistive technologies", () => {
    render(<IconButton icon={<span />} aria-label="Split" tooltip />);
    const button = screen.getByRole("button", { name: "Split" });
    fireEvent.focus(button);
    expect(screen.getByTestId("icon-button-label")).toHaveAttribute("aria-hidden", "true");
    fireEvent.keyDown(button, { key: "Escape" });
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
    fireEvent.mouseEnter(button);
    fireEvent.scroll(window);
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
  });

  it("closes when the button becomes disabled", () => {
    const { rerender } = render(<IconButton icon={<span />} aria-label="Lots" tooltip />);
    fireEvent.mouseEnter(screen.getByRole("button", { name: "Lots" }));
    rerender(<IconButton icon={<span />} aria-label="Lots" tooltip disabled />);
    expect(screen.queryByTestId("icon-button-label")).toBeNull();
  });

  it("still calls the caller's own handlers", () => {
    const onMouseEnter = vi.fn();
    const onFocus = vi.fn();
    render(
      <IconButton
        icon={<span />}
        aria-label="Fee"
        tooltip
        onMouseEnter={onMouseEnter}
        onFocus={onFocus}
      />,
    );
    const button = screen.getByRole("button", { name: "Fee" });
    fireEvent.mouseEnter(button);
    fireEvent.focus(button);
    expect(onMouseEnter).toHaveBeenCalledTimes(1);
    expect(onFocus).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("icon-button-label")).toHaveTextContent("Fee");
  });
});

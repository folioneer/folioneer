import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Dialog } from "./Dialog";
import { ModalContainer } from "./ModalContainer";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe("ModalContainer", () => {
  // F24 — backdrop dismiss button must carry an i18n-resolved aria-label,
  // not the hardcoded English "Close modal" string.
  it("renders the backdrop dismiss button with a translated aria-label", () => {
    render(
      <ModalContainer isOpen onClose={vi.fn()}>
        <div>child</div>
      </ModalContainer>,
    );
    // Mock returns the key verbatim — assert the key (proxy for "value flowed through t()").
    expect(screen.getByRole("button", { name: "action.close" })).toBeInTheDocument();
  });

  it("does not carry the legacy hardcoded English 'Close modal' aria-label", () => {
    render(
      <ModalContainer isOpen onClose={vi.fn()}>
        <div>child</div>
      </ModalContainer>,
    );
    expect(screen.queryByRole("button", { name: "Close modal" })).not.toBeInTheDocument();
  });

  // IMP-052 — the asset form over the import window: Escape leaves the window open.
  it("closes only the top modal of a stack on Escape", async () => {
    const closeUnder = vi.fn();
    const closeOver = vi.fn();
    const user = userEvent.setup();
    const stack = (over: boolean) => (
      <>
        <ModalContainer isOpen onClose={closeUnder}>
          under
        </ModalContainer>
        <ModalContainer isOpen={over} onClose={closeOver}>
          over
        </ModalContainer>
      </>
    );
    const { rerender, unmount } = render(stack(false));
    rerender(stack(true));

    await user.keyboard("{Escape}");
    expect(closeOver).toHaveBeenCalledOnce();
    expect(closeUnder).not.toHaveBeenCalled();

    rerender(stack(false));
    expect(document.body.style.overflow).toBe("hidden");
    await user.keyboard("{Escape}");
    expect(closeUnder).toHaveBeenCalledOnce();

    unmount();
    expect(document.body.style.overflow).toBe("auto");
  });

  // IMP-052 — the asset form is a Dialog, which has no Escape: the window under it stays.
  it("leaves Escape to a Dialog opened over it", async () => {
    const closeUnder = vi.fn();
    const user = userEvent.setup();
    render(
      <>
        <ModalContainer isOpen onClose={closeUnder}>
          under
        </ModalContainer>
        <Dialog isOpen onClose={vi.fn()} title="over">
          over
        </Dialog>
      </>,
    );
    await user.keyboard("{Escape}");
    expect(closeUnder).not.toHaveBeenCalled();
  });

  it("renders nothing when isOpen is false", () => {
    const { container } = render(
      <ModalContainer isOpen={false} onClose={vi.fn()}>
        <div>child</div>
      </ModalContainer>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("invokes onClose when the backdrop is clicked", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <ModalContainer isOpen onClose={onClose}>
        <div>child</div>
      </ModalContainer>,
    );
    await user.click(screen.getByRole("button", { name: "action.close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("invokes onClose when Escape is pressed while open", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <ModalContainer isOpen onClose={onClose}>
        <div>child</div>
      </ModalContainer>,
    );
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

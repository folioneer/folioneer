import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmationDialog, Dialog } from "./Dialog";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("Dialog — stable ids (F25)", () => {
  it("applies the id to the dialog surface and the close button", () => {
    render(
      <Dialog id="sample-dialog" isOpen onClose={vi.fn()} title="Title">
        content
      </Dialog>,
    );
    const surface = document.querySelector("#sample-dialog");
    expect(surface).not.toBeNull();
    expect(surface?.getAttribute("role")).toBe("dialog");
    expect(document.querySelector("#modal-close-btn")).not.toBeNull();
  });

  it("derives the ConfirmationDialog surface id from confirmId", () => {
    render(
      <ConfirmationDialog
        isOpen
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
        title="T"
        message="M"
        confirmLabel="OK"
        cancelLabel="No"
        confirmId="confirm-delete-x"
      />,
    );
    expect(document.querySelector("#confirm-delete-x-dialog")).not.toBeNull();
    expect(document.querySelector("#confirm-delete-x")).not.toBeNull();
  });
});

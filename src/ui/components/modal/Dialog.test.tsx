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

// ACC-014 / AST-014 (#446) — a refused confirmation is told inside the dialog that asked,
// beside its actions.
describe("ConfirmationDialog — a refusal inside the dialog", () => {
  it("shows the error inside the dialog, with a stable id", () => {
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
        error="Refused"
      />,
    );
    const error = document.querySelector("#confirm-delete-x-error");
    expect(error?.textContent).toBe("Refused");
    expect(error?.getAttribute("role")).toBe("alert");
    expect(document.querySelector("#confirm-delete-x-dialog")?.contains(error ?? null)).toBe(true);
  });

  it("shows no error element without an error", () => {
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
    expect(document.querySelector("#confirm-delete-x-error")).toBeNull();
  });
});

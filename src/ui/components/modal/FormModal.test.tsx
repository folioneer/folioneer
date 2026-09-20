import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FormModal } from "./FormModal";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("FormModal — stable ids (F25)", () => {
  it("applies the id to the modal panel and the close button", () => {
    render(
      <FormModal id="sample-modal" isOpen onClose={vi.fn()} title="Title">
        content
      </FormModal>,
    );
    const panel = document.querySelector("#sample-modal");
    expect(panel).not.toBeNull();
    expect(panel?.getAttribute("role")).toBe("dialog");
    expect(document.querySelector("#modal-close-btn")).not.toBeNull();
  });
});

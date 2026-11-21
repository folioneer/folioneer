import { configure, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

configure({ testIdAttribute: "id" });

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { StartupScreen } from "./StartupScreen";

describe("StartupScreen", () => {
  // #873's mock — the brand over its watermark, the message beside a small spinner.
  it("shows the brand and the message while the portfolio opens", () => {
    render(<StartupScreen />);

    expect(screen.getByTestId("startup-brand").textContent).toBe("Folioneer");
    expect(screen.getByTestId("startup-message").textContent).toBe("app.starting");
    const watermark = screen.getByTestId("startup-watermark");
    expect(watermark.textContent).toBe("Folioneer");
    expect(watermark.getAttribute("aria-hidden")).toBe("true");
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

const hook = vi.fn();
vi.mock("./useAgentsAllowed", () => ({ useAgentsAllowed: () => hook() }));

import { AgentsAllowedSetting } from "./AgentsAllowedSetting";

const setAllowed = vi.fn();
const state = (overrides = {}) => ({
  isReady: true,
  available: true,
  allowed: false,
  isSaving: false,
  setAllowed,
  ...overrides,
});

describe("AgentsAllowedSetting", () => {
  beforeEach(() => vi.clearAllMocks());

  // AGT-022 — off by default, with what switching it on means; the switch goes to the core.
  it("shows the setting off with its meaning and sends a switch", () => {
    hook.mockReturnValue(state());
    render(<AgentsAllowedSetting />);

    const box = screen.getByLabelText(/settings.agents_allowed_label/);
    expect(box).toHaveAttribute("id", "settings-agents-allowed");
    expect(box).not.toBeChecked();
    expect(screen.getByText("settings.agents_allowed_description")).toBeInTheDocument();

    fireEvent.click(box);
    expect(setAllowed).toHaveBeenCalledWith(true);
  });

  // AGT-023 — on a system without an agent connection the setting cannot be switched and
  // says so.
  it("cannot be switched on a system without an agent connection", () => {
    hook.mockReturnValue(state({ available: false }));
    render(<AgentsAllowedSetting />);

    expect(screen.getByLabelText(/settings.agents_allowed_label/)).toBeDisabled();
    expect(screen.getByText("settings.agents_unavailable_description")).toBeInTheDocument();
  });

  it("shows nothing until the setting is read", () => {
    hook.mockReturnValue(state({ isReady: false }));
    const { container } = render(<AgentsAllowedSetting />);
    expect(container).toBeEmptyDOMElement();
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WhatsNewDialogMount } from "./WhatsNewDialogMount";

const { mockUseWhatsNewDialog, mockDismiss, mockNavigate, mockSearch } = vi.hoisted(() => ({
  mockUseWhatsNewDialog: vi.fn(),
  mockDismiss: vi.fn(),
  mockNavigate: vi.fn(),
  mockSearch: { value: {} as Record<string, unknown> },
}));

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mockNavigate,
  useSearch: () => mockSearch.value,
}));

vi.mock("@/features/whats_new/useWhatsNewDialog", () => ({
  useWhatsNewDialog: (changelogText: string) => mockUseWhatsNewDialog(changelogText),
}));

// Stub the dialog so the mount renders in isolation; surface its props.
vi.mock("@/features/whats_new/WhatsNewDialog", () => ({
  WhatsNewDialog: ({
    version,
    sections,
    onDismiss,
  }: {
    version: string;
    sections: { version: string }[];
    onDismiss: () => void;
  }) => (
    <button type="button" data-testid="whats-new-dialog" onClick={onDismiss}>
      {version}:{sections.length}
    </button>
  ),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

// A fixed changelog stands in for the bundled one, so WNW-080 never depends on
// which versions have been released.
const RELEASED_VERSION = "0.34.0";
vi.mock("../../../CHANGELOG.md?raw", () => ({
  default: "# Changelog\n\n## [0.34.0] - 2026-07-06\n\n### Added\n\n- a released change\n",
}));

describe("WhatsNewDialogMount (WNW-020/030/050/080)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSearch.value = {};
  });

  it("renders nothing while the hook reports no sections (WNW-030)", () => {
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: "0.33.0",
      sections: null,
      dismiss: mockDismiss,
    });
    const { container } = render(<WhatsNewDialogMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("feeds the bundled changelog text into the hook", () => {
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: "0.33.0",
      sections: null,
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    const changelogText = mockUseWhatsNewDialog.mock.calls[0]?.[0];
    expect(changelogText).toContain("Changelog");
  });

  it("renders the dialog with the resolved version and sections when visible (WNW-020)", () => {
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: "0.33.0",
      sections: [{ version: "0.33.0" }, { version: "0.32.0" }],
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    expect(screen.getByTestId("whats-new-dialog")).toHaveTextContent("0.33.0:2");
  });

  it("wires the dialog's dismiss back to the hook (WNW-050)", () => {
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: "0.33.0",
      sections: [{ version: "0.33.0" }],
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    fireEvent.click(screen.getByTestId("whats-new-dialog"));
    expect(mockDismiss).toHaveBeenCalledTimes(1);
  });

  it("opens the current version's section on demand via ?modal=whats-new (WNW-080)", () => {
    mockSearch.value = { modal: "whats-new" };
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: RELEASED_VERSION,
      sections: null,
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    expect(screen.getByTestId("whats-new-dialog")).toHaveTextContent(`${RELEASED_VERSION}:1`);
  });

  it("dismissing the on-demand dialog clears the URL param without acknowledging (WNW-080)", () => {
    mockSearch.value = { modal: "whats-new" };
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: RELEASED_VERSION,
      sections: null,
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    fireEvent.click(screen.getByTestId("whats-new-dialog"));
    expect(mockDismiss).not.toHaveBeenCalled();
    expect(mockNavigate).toHaveBeenCalledTimes(1);
  });

  it("the launch dialog takes precedence over the on-demand request (WNW-080)", () => {
    mockSearch.value = { modal: "whats-new" };
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: RELEASED_VERSION,
      sections: [{ version: RELEASED_VERSION }, { version: "0.33.2" }],
      dismiss: mockDismiss,
    });
    render(<WhatsNewDialogMount />);
    expect(screen.getByTestId("whats-new-dialog")).toHaveTextContent(`${RELEASED_VERSION}:2`);
    fireEvent.click(screen.getByTestId("whats-new-dialog"));
    expect(mockDismiss).toHaveBeenCalledTimes(1);
  });

  it("renders nothing on demand when the current version has no changelog section", () => {
    mockSearch.value = { modal: "whats-new" };
    mockUseWhatsNewDialog.mockReturnValue({
      appVersion: "999.0.0",
      sections: null,
      dismiss: mockDismiss,
    });
    const { container } = render(<WhatsNewDialogMount />);
    expect(container).toBeEmptyDOMElement();
  });
});

import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncIndicator } from "./SyncIndicator";

const { mockUseSyncIndicator } = vi.hoisted(() => ({ mockUseSyncIndicator: vi.fn() }));

vi.mock("./useSyncIndicator", () => ({ useSyncIndicator: () => mockUseSyncIndicator() }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const makeState = (overrides: Record<string, unknown> = {}) => ({
  isLoading: false,
  visible: false,
  lastSyncCompletedAt: null,
  needsAttention: false,
  ...overrides,
});

describe("SyncIndicator — visibility (SYN-010/063)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSyncIndicator.mockReturnValue(makeState());
  });

  it("renders nothing when sync is disabled", () => {
    render(<SyncIndicator />);
    expect(screen.queryByTestId("sync-indicator")).toBeNull();
  });

  it("shows the last-sync time when enabled", () => {
    mockUseSyncIndicator.mockReturnValue(
      makeState({ visible: true, lastSyncCompletedAt: "2026-08-20T10:00:00Z" }),
    );
    render(<SyncIndicator />);

    expect(screen.getByTestId("sync-indicator")).toHaveTextContent(
      'sync.indicator.last_sync:{"when":"',
    );
  });

  it("SYN-063: says that the computers never synced when there is no last sync", () => {
    mockUseSyncIndicator.mockReturnValue(makeState({ visible: true }));
    render(<SyncIndicator />);

    expect(screen.getByTestId("sync-indicator")).toHaveTextContent("sync.indicator.never");
  });

  it("SYN-063: the attention badge says on hover that sync needs attention", () => {
    mockUseSyncIndicator.mockReturnValue(makeState({ visible: true, needsAttention: true }));
    render(<SyncIndicator />);

    expect(screen.getByTestId("sync-indicator-attention")).toHaveAttribute(
      "title",
      "sync.indicator.attention",
    );
  });

  it("shows an attention badge when needsAttention is true", () => {
    mockUseSyncIndicator.mockReturnValue(makeState({ visible: true, needsAttention: true }));
    render(<SyncIndicator />);

    expect(screen.getByTestId("sync-indicator-attention")).toBeInTheDocument();
  });

  it("does not show an attention badge when needsAttention is false", () => {
    mockUseSyncIndicator.mockReturnValue(makeState({ visible: true, needsAttention: false }));
    render(<SyncIndicator />);

    expect(screen.queryByTestId("sync-indicator-attention")).toBeNull();
  });
});

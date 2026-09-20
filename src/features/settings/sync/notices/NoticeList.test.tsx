import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConflictNotice } from "@/bindings";
import { NoticeList } from "./NoticeList";

const { mockUseNoticeList } = vi.hoisted(() => ({ mockUseNoticeList: vi.fn() }));

vi.mock("./useNoticeList", () => ({ useNoticeList: () => mockUseNoticeList() }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}));

function makeNotice(overrides: Partial<ConflictNotice> = {}): ConflictNotice {
  return {
    notice_id: "notice-1",
    kind: "OverruledEdit",
    record_kind: "Transaction",
    record_identity: "tx-1",
    record_label: "Sell 10 AAPL on 2026-08-01",
    other_device_id: "device-2",
    other_device_name: "Laptop",
    raised_at: "2026-08-20T10:00:00Z",
    ...overrides,
  };
}

const makeState = (overrides: Record<string, unknown> = {}) => ({
  handleDismiss: vi.fn(),
  dismissError: null,
  ...overrides,
});

describe("NoticeList — undismissed notices (SYN-066)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseNoticeList.mockReturnValue(makeState());
  });

  it("renders a dismiss action per notice with a stable id (F25)", () => {
    render(
      <NoticeList
        notices={[makeNotice({ notice_id: "notice-1" }), makeNotice({ notice_id: "notice-2" })]}
        onDismissed={vi.fn()}
      />,
    );

    expect(screen.getByTestId("sync-notice-dismiss-notice-1")).toBeInTheDocument();
    expect(screen.getByTestId("sync-notice-dismiss-notice-2")).toBeInTheDocument();
  });

  it("calls handleDismiss with the notice id when its dismiss action is clicked", () => {
    const handleDismiss = vi.fn();
    mockUseNoticeList.mockReturnValue(makeState({ handleDismiss }));
    render(<NoticeList notices={[makeNotice({ notice_id: "notice-1" })]} onDismissed={vi.fn()} />);

    fireEvent.click(screen.getByTestId("sync-notice-dismiss-notice-1"));

    expect(handleDismiss).toHaveBeenCalledWith("notice-1");
  });

  it("renders the presented error key when a dismiss fails (NoticeNotFound, F27)", () => {
    mockUseNoticeList.mockReturnValue(
      makeState({ dismissError: { key: "sync.errors.NoticeNotFound" } }),
    );
    render(<NoticeList notices={[makeNotice()]} onDismissed={vi.fn()} />);

    expect(screen.getByText("sync.errors.NoticeNotFound")).toBeInTheDocument();
  });
});

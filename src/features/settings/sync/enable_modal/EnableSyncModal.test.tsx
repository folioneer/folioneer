import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EnableSyncModal } from "./EnableSyncModal";

// Controlled orchestration hook (mocked per task direction, mirrors
// ScheduledFetchSection.test.tsx's hook-mocking pattern).
const { mockUseEnableSyncModal } = vi.hoisted(() => ({ mockUseEnableSyncModal: vi.fn() }));

vi.mock("./useEnableSyncModal", () => ({ useEnableSyncModal: () => mockUseEnableSyncModal() }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const makeState = (overrides: Record<string, unknown> = {}) => ({
  step: 1,
  folder: "",
  setFolder: vi.fn(),
  handleBrowse: vi.fn(),
  folderError: null,
  canProceedToStep2: false,
  goToStep2: vi.fn(),
  passphrase: "",
  setPassphrase: vi.fn(),
  passphraseConfirm: "",
  setPassphraseConfirm: vi.fn(),
  passphraseMismatch: false,
  passphraseTooShort: false,
  deviceName: "",
  setDeviceName: vi.fn(),
  isJoin: false,
  canSubmit: false,
  isSubmitting: false,
  submitError: null,
  handleSubmit: vi.fn(),
  confirmingStartOver: false,
  confirmStartOver: vi.fn(),
  cancelStartOver: vi.fn(),
  ...overrides,
});

describe("EnableSyncModal — step 1 folder field (SYN-011/019, D11)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseEnableSyncModal.mockReturnValue(makeState());
  });

  it("renders the folder text field and a Browse button", () => {
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-folder")).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable-browse")).toBeInTheDocument();
  });

  it("calls handleBrowse (which fills the field) when Browse is clicked", () => {
    const handleBrowse = vi.fn();
    mockUseEnableSyncModal.mockReturnValue(makeState({ handleBrowse }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    fireEvent.click(screen.getByTestId("sync-enable-browse"));

    expect(handleBrowse).toHaveBeenCalled();
  });

  it("calls setFolder when the field is edited", () => {
    const setFolder = vi.fn();
    mockUseEnableSyncModal.mockReturnValue(makeState({ setFolder }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    fireEvent.change(screen.getByTestId("sync-enable-folder"), {
      target: { value: "/home/user/sync" },
    });

    expect(setFolder).toHaveBeenCalledWith("/home/user/sync");
  });

  it("shows an inline folder error and blocks the next step when the folder has a problem", () => {
    mockUseEnableSyncModal.mockReturnValue(
      makeState({ folderError: { key: "sync.folder_problem.Missing" }, canProceedToStep2: false }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByText("sync.folder_problem.Missing")).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable-next")).toBeDisabled();
  });
});

describe("EnableSyncModal — step 2 passphrase (SYN-011/012/014/015/019)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows two passphrase fields for a first device (holds_portfolio false)", () => {
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2, isJoin: false }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-passphrase")).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable-passphrase-confirm")).toBeInTheDocument();
  });

  it("shows only one passphrase field when joining (holds_portfolio true)", () => {
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2, isJoin: true }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-passphrase")).toBeInTheDocument();
    expect(screen.queryByTestId("sync-enable-passphrase-confirm")).toBeNull();
  });

  it("blocks the next step and shows the fresh-install message when installation_holds_user_data is true (SYN-014)", () => {
    mockUseEnableSyncModal.mockReturnValue(
      makeState({
        isJoin: true,
        folderError: { key: "sync.errors.InstallationHoldsUserData" },
        canProceedToStep2: false,
      }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByText("sync.errors.InstallationHoldsUserData")).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable-next")).toBeDisabled();
  });

  it("blocks the next step and shows the update message when the format is not readable (SYN-019/035)", () => {
    mockUseEnableSyncModal.mockReturnValue(
      makeState({
        isJoin: true,
        folderError: { key: "sync.errors.UpdateRequired", vars: { dataFormatVersion: 9 } },
        canProceedToStep2: false,
      }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(
      screen.getByText('sync.errors.UpdateRequired:{"dataFormatVersion":9}'),
    ).toBeInTheDocument();
    expect(screen.getByTestId("sync-enable-next")).toBeDisabled();
  });

  it("blocks submission when the passphrase is shorter than 12 characters (SYN-012)", () => {
    mockUseEnableSyncModal.mockReturnValue(
      makeState({ step: 2, passphraseTooShort: true, canSubmit: false }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-submit")).toBeDisabled();
  });

  it("blocks submission on a passphrase mismatch", () => {
    mockUseEnableSyncModal.mockReturnValue(
      makeState({ step: 2, passphraseMismatch: true, canSubmit: false }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-submit")).toBeDisabled();
  });

  it("requires the device name field", () => {
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2 }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByTestId("sync-enable-device-name")).toBeRequired();
  });

  it("submits via the sync-enable-form form id", () => {
    const handleSubmit = vi.fn((e?: { preventDefault: () => void }) => e?.preventDefault());
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2, canSubmit: true, handleSubmit }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    fireEvent.submit(screen.getByTestId("sync-enable-form"));

    expect(handleSubmit).toHaveBeenCalled();
  });

  it("renders the no-recovery (SYN-053) and metadata-exposure (SYN-054) statements", () => {
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2 }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="enable" />);

    expect(screen.getByText("sync.no_recovery_note")).toBeInTheDocument();
    expect(screen.getByText("sync.metadata_exposure_note")).toBeInTheDocument();
  });
});

describe("EnableSyncModal — start-over variant (SYN-071)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows its own confirmation before calling startSyncOver", () => {
    mockUseEnableSyncModal.mockReturnValue(makeState({ step: 2, confirmingStartOver: true }));
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="start-over" />);

    expect(screen.getByTestId("sync-start-over-confirm")).toBeInTheDocument();
  });

  it("calls confirmStartOver when the start-over confirmation is accepted", () => {
    const confirmStartOver = vi.fn();
    mockUseEnableSyncModal.mockReturnValue(
      makeState({ step: 2, confirmingStartOver: true, confirmStartOver }),
    );
    render(<EnableSyncModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} variant="start-over" />);

    fireEvent.click(screen.getByTestId("sync-start-over-confirm"));

    expect(confirmStartOver).toHaveBeenCalled();
  });
});

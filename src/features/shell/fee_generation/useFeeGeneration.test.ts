import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useFeeGeneration } from "./useFeeGeneration";

const applyDueFeeDeductions = vi.fn();
vi.mock("../gateway", () => ({
  shellGateway: {
    applyDueFeeDeductions: () => applyDueFeeDeductions(),
  },
}));

const showSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => showSnackbar,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("useFeeGeneration (FEE-040)", () => {
  beforeEach(() => {
    applyDueFeeDeductions.mockReset();
    showSnackbar.mockReset();
  });

  it("fires applyDueFeeDeductions once on mount and stays silent on success", async () => {
    applyDueFeeDeductions.mockResolvedValue({ status: "ok", data: null });
    renderHook(() => useFeeGeneration());
    await waitFor(() => expect(applyDueFeeDeductions).toHaveBeenCalledTimes(1));
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  it("surfaces an error snackbar when generation fails (F27)", async () => {
    applyDueFeeDeductions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    renderHook(() => useFeeGeneration());
    await waitFor(() =>
      expect(showSnackbar).toHaveBeenCalledWith("fee_generation.apply_error", "error"),
    );
  });
});

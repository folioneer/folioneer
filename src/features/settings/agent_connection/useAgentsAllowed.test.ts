import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../gateway", () => ({
  getAgentConnectionState: vi.fn(),
  setAgentsAllowed: vi.fn(),
}));

const showSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => showSnackbar }));
// One `t` for every render, as react-i18next gives: the hook reads the setting again only
// when it changes.
vi.mock("react-i18next", () => {
  const t = (key: string) => key;
  return { useTranslation: () => ({ t }) };
});
vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

import * as gateway from "../gateway";
import { useAgentsAllowed } from "./useAgentsAllowed";

const state = (allowed: boolean, available = true) => ({
  available,
  allowed,
  user: "phil",
  requests: [],
  sessions: [],
});

describe("useAgentsAllowed", () => {
  beforeEach(() => vi.clearAllMocks());

  // AGT-022 — the setting is the core's: read at mount, shown as answered after a switch.
  it("reads the setting and shows what the core answers to a switch", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state(false));
    vi.mocked(gateway.setAgentsAllowed).mockResolvedValue({ status: "ok", data: state(true) });

    const { result } = renderHook(() => useAgentsAllowed());
    expect(result.current.isReady).toBe(false);
    await waitFor(() => expect(result.current.isReady).toBe(true));
    expect(result.current.allowed).toBe(false);
    expect(result.current.available).toBe(true);

    act(() => result.current.setAllowed(true));

    await waitFor(() => expect(result.current.allowed).toBe(true));
    expect(gateway.setAgentsAllowed).toHaveBeenCalledWith(true);
    expect(result.current.isSaving).toBe(false);
    expect(showSnackbar).not.toHaveBeenCalled();
  });

  // AGT-023 — a refused switch leaves the setting as it was and says why.
  it("keeps the setting and says why when the core refuses", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state(false, false));
    vi.mocked(gateway.setAgentsAllowed).mockResolvedValue({
      status: "error",
      error: { code: "NoAgentChannel" },
    });

    const { result } = renderHook(() => useAgentsAllowed());
    await waitFor(() => expect(result.current.isReady).toBe(true));
    expect(result.current.available).toBe(false);

    act(() => result.current.setAllowed(true));

    await waitFor(() => expect(showSnackbar).toHaveBeenCalledWith("error.NoAgentChannel", "error"));
    expect(result.current.allowed).toBe(false);
  });

  it("says so when the setting cannot be read", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockRejectedValue(new Error("ipc"));

    const { result } = renderHook(() => useAgentsAllowed());

    await waitFor(() => expect(showSnackbar).toHaveBeenCalledWith("error.Unknown", "error"));
    expect(result.current.isReady).toBe(false);
  });

  it("says so when the switch cannot be sent", async () => {
    vi.mocked(gateway.getAgentConnectionState).mockResolvedValue(state(true));
    vi.mocked(gateway.setAgentsAllowed).mockRejectedValue(new Error("ipc"));

    const { result } = renderHook(() => useAgentsAllowed());
    await waitFor(() => expect(result.current.isReady).toBe(true));
    act(() => result.current.setAllowed(false));

    await waitFor(() => expect(showSnackbar).toHaveBeenCalledWith("error.Unknown", "error"));
    expect(result.current.allowed).toBe(true);
  });
});

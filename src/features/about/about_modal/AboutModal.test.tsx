import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => vi.fn() }));
vi.mock("../about_page/useAboutPage", () => ({
  useAboutPage: () => ({ checkStatus: "idle", handleCheckForUpdate: vi.fn() }),
}));

const { AboutModal } = await import("./AboutModal");
const { useAppStore } = await import("@/lib/store");

describe("AboutModal — the build's channel beside its version (UPD-031)", () => {
  afterEach(() => {
    useAppStore.setState({ distributionChannel: null });
  });

  it("shows no channel in the public build", () => {
    useAppStore.setState({ appVersion: "0.2.0", distributionChannel: null });
    render(<AboutModal isOpen onClose={vi.fn()} />);

    expect(screen.queryByTestId("about-channel")).toBeNull();
  });

  it("names the channel of a build that belongs to one", () => {
    useAppStore.setState({ appVersion: "0.2.0", distributionChannel: "private" });
    render(<AboutModal isOpen onClose={vi.fn()} />);

    expect(screen.getByTestId("about-channel")).toHaveTextContent("about.channel_private");
  });
});

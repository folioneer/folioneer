import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, string>) => {
      const { defaultValue: _defaultValue, ...vars } = options ?? {};
      const values = Object.values(vars);
      return values.length > 0 ? `${key}(${values.join(",")})` : key;
    },
  }),
}));

const { SidebarVersion } = await import("./SidebarVersion");

describe("SidebarVersion — the version in the sidebar footer (UPD-031)", () => {
  it("shows the version alone in the public build", () => {
    render(<SidebarVersion appVersion="0.2.0" distributionChannel={null} isOpen />);

    expect(screen.getByText("shell.sidebar_version(0.2.0)")).toBeInTheDocument();
  });

  it("names the channel after the version when open", () => {
    render(<SidebarVersion appVersion="0.2.0" distributionChannel="private" isOpen />);

    expect(
      screen.getByText("shell.sidebar_version_channel(0.2.0,shell.channel_private)"),
    ).toBeInTheDocument();
  });

  it("names the channel on a second line when collapsed", () => {
    render(<SidebarVersion appVersion="0.2.0" distributionChannel="private" isOpen={false} />);

    expect(screen.getByText("v0.2.0")).toBeInTheDocument();
    expect(screen.getByText("shell.channel_private")).toBeInTheDocument();
  });
});

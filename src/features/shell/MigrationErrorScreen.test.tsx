import { configure, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

configure({ testIdAttribute: "id" });

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}));

import { MigrationErrorScreen } from "./MigrationErrorScreen";

describe("MigrationErrorScreen", () => {
  // UPD-035 — the copy was put back: nothing lost, the copies' folder named, the error shown.
  it("says nothing was lost and names the copies' folder after a put-back", () => {
    render(
      <MigrationErrorScreen
        failure={{ detail: "migration 45 failed", backups_dir: "/data/folioneer/backups" }}
      />,
    );

    expect(screen.getByTestId("migration-error-message").textContent).toBe(
      "app.migration_restored",
    );
    expect(screen.getByText("app.migration_backups")).toBeTruthy();
    expect(screen.getByTestId("migration-error-backups").textContent).toBe(
      "/data/folioneer/backups",
    );
    expect(screen.getByTestId("migration-error-detail").textContent).toBe("migration 45 failed");
  });

  // UPD-035 — any other startup failure: the plain message and the error, no folder.
  it("says the plain message when nothing was put back", () => {
    render(<MigrationErrorScreen failure={{ detail: "disk full", backups_dir: null }} />);

    expect(screen.getByTestId("migration-error-message").textContent).toBe("app.migration_error");
    expect(screen.queryByTestId("migration-error-backups")).toBeNull();
    expect(screen.getByTestId("migration-error-detail").textContent).toBe("disk full");
  });
});

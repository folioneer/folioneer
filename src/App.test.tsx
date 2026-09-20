import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import App from "./App";

// Mock Tauri event system (used by useUpdateBanner and db:migration_error listener)
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock Tauri bindings
vi.mock("./bindings", () => ({
  commands: {
    getAssets: vi.fn(() => Promise.resolve({ status: "ok", data: [] })),
    getAssetsWithArchived: vi.fn(() => Promise.resolve({ status: "ok", data: [] })),
    addAsset: vi.fn(),
    deleteAsset: vi.fn(),
    getAccountTypes: vi.fn(() => Promise.resolve({ status: "ok", data: [] })),
    addAccountType: vi.fn(),
    deleteAccountType: vi.fn(),
    getAccounts: vi.fn(() => Promise.resolve({ status: "ok", data: [] })),
    getCategories: vi.fn(() => Promise.resolve({ status: "ok", data: [] })),
    checkForUpdate: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    downloadUpdate: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    installUpdate: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    applyDueFeeDeductions: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    getPriceFreshness: vi.fn(() =>
      Promise.resolve({ status: "ok", data: { newest_price_date: null, last_fetch_at: null } }),
    ),
    getSyncStatus: vi.fn(() =>
      Promise.resolve({
        status: "ok",
        data: {
          enabled: false,
          paused: false,
          device_id: null,
          device_name: null,
          folder: null,
          last_sync_completed_at: null,
          roster: [],
          held_back_count: 0,
          oldest_held_back_since: null,
          notices: [],
          inconsistent_holdings: [],
          failures: [],
        },
      }),
    ),
  },
  events: {
    event: {
      listen: vi.fn(() => Promise.resolve(() => {})),
    },
  },
}));

test("renders Folioneer title", async () => {
  render(<App />);
  const titleElements = await screen.findAllByText(/Folioneer/i);
  expect(titleElements.length).toBeGreaterThan(0);
});

test("renders Assets navigation item", async () => {
  render(<App />);
  const assetsElements = await screen.findAllByText(/Assets/i);
  expect(assetsElements.length).toBeGreaterThan(0);
});

test("renders Categories navigation item", async () => {
  render(<App />);
  const categoriesElements = await screen.findAllByText(/Categories/i);
  expect(categoriesElements.length).toBeGreaterThan(0);
});

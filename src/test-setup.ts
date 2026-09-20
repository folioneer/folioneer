import "@testing-library/jest-dom";
import { vi } from "vitest";

// Stub the Tauri runtime globally. `bindings.ts` imports `invoke`/`Channel` at
// module load and `lib/logger.ts` chains `.catch()` on every `invoke` call, so
// the mock must return a Promise — `vi.fn()` returning `undefined` would crash.
// Tests that need a typed `invoke` mock can override this via per-file
// `vi.mock("@tauri-apps/api/core", ...)`.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  Channel: vi.fn(),
}));

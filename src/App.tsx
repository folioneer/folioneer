import { RouterProvider } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import type { MigrationFailure } from "@/bindings";
import { accountGateway } from "@/features/accounts/gateway";
import { refreshCurrencyRates } from "@/features/currency/gateway";
import { shellGateway } from "@/features/shell/gateway";
import { MigrationErrorScreen } from "@/features/shell/MigrationErrorScreen";
import { StartupScreen } from "@/features/shell/StartupScreen";
import { getAutoFetch } from "@/lib/autoFetchStorage";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import { router } from "./router";

/**
 * MKT-121 launch auto-fetch decision, extracted from the mount effect so it is
 * unit-testable (the effect just awaits it once after init). Fire-and-forget:
 * dispatch-level failures are logged, never surfaced as a cold-start snackbar.
 *
 * Gated on an External provider being chosen (MKT-212) and on the auto-fetch setting
 * (MKT-120).
 */
export async function maybeLaunchAutoFetch(): Promise<void> {
  // MKT-212 — a computer with no External provider chosen has no auto-fetch to start.
  if (!useAppStore.getState().hasExternalProvider) return;
  if (!getAutoFetch()) return;
  try {
    // PMV-010 — the launch auto-fetch states which action started it; only a
    // user-started Global refresh reports movement.
    const result = await accountGateway.fetchAllAssetPrices("Launch");
    if (result.status === "error") {
      logger.warn("[App] auto-fetch dispatch returned error", { code: result.error.code });
    }
  } catch (error) {
    logger.error("[App] auto-fetch dispatch threw", { error });
  }
}

/**
 * FXR-075 — the launch rate refresh: every build, whatever the auto-fetch setting.
 * Silent — an error is logged, never shown.
 */
export async function launchRateRefresh(): Promise<void> {
  try {
    const result = await refreshCurrencyRates();
    if (result.status === "error") {
      logger.warn("[App] rate refresh returned error", { code: result.error.code });
    }
  } catch (error) {
    logger.error("[App] rate refresh threw", { error });
  }
}

function App() {
  const [dbError, setDbError] = useState<MigrationFailure | null>(null);
  const init = useAppStore((state) => state.init);
  const isInitialized = useAppStore((state) => state.isInitialized);

  useEffect(() => {
    logger.info("[App] mounted");
    // UPD-018 / UPD-018A — listen for the critical startup failure from the backend, and read
    // the one that happened before the window listened.
    const unlistenPromise = shellGateway.onMigrationError((failure) => {
      setDbError(failure);
    });
    let cancelled = false;
    shellGateway
      .getStartupFailure()
      .then((failure) => {
        if (failure && !cancelled) setDbError(failure);
      })
      .catch((error) => logger.error("[App] startup failure unreadable", { error }));
    return () => {
      cancelled = true;
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    return init();
  }, [init]);

  // MKT-121 — fire-and-forget auto-fetch when the user has enabled the setting.
  // Runs once after init completes; per-asset results arrive via AssetPriceUpdated events.
  // Dispatch-level failures (FetchAlreadyRunning, NoFetchableHoldings, DatabaseError) are
  // logged server-side via the FE logger — no startup snackbar to avoid noise on launch.
  useEffect(() => {
    if (!isInitialized) return;
    // MKT-121 — decision lives in maybeLaunchAutoFetch (unit-tested).
    void maybeLaunchAutoFetch();
    void launchRateRefresh();
  }, [isInitialized]);

  // UPD-018 — critical startup failure: app blocked with the error screen (UPD-035)
  if (dbError) {
    return <MigrationErrorScreen failure={dbError} />;
  }

  // #873's mock — the startup screen while migrations/init are running
  if (!isInitialized) {
    return <StartupScreen />;
  }

  return <RouterProvider router={router} />;
}

export default App;

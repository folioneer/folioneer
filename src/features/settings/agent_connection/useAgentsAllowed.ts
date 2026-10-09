import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { getAgentConnectionState, setAgentsAllowed } from "../gateway";

export interface UseAgentsAllowedResult {
  /** False until the setting is read. */
  isReady: boolean;
  /** Whether this system has an agent connection (AGT-023). */
  available: boolean;
  /** Whether the owner allows agents to connect (AGT-022). */
  allowed: boolean;
  isSaving: boolean;
  setAllowed: (allowed: boolean) => void;
}

/**
 * AGT-022 — the owner's setting "Allow agents to connect": read from the core, switched
 * through it, and shown as the core answers.
 */
export function useAgentsAllowed(): UseAgentsAllowedResult {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [state, setState] = useState<{ available: boolean; allowed: boolean } | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let isCurrent = true;
    const read = async () => {
      try {
        const read = await getAgentConnectionState();
        if (isCurrent) setState({ available: read.available, allowed: read.allowed });
      } catch (e) {
        logger.error("[useAgentsAllowed] setting not read", { error: e });
        if (isCurrent) showSnackbar(t("error.Unknown"), "error");
      }
    };
    void read();
    return () => {
      isCurrent = false;
    };
  }, [showSnackbar, t]);

  const setAllowed = useCallback(
    (allowed: boolean) => {
      setIsSaving(true);
      setAgentsAllowed(allowed)
        .then((result) => {
          if (result.status === "ok") {
            setState({ available: result.data.available, allowed: result.data.allowed });
          } else {
            showSnackbar(t(`error.${result.error.code}`), "error");
          }
        })
        .catch((e) => {
          logger.error("[useAgentsAllowed] setting not switched", { error: e });
          showSnackbar(t("error.Unknown"), "error");
        })
        .finally(() => setIsSaving(false));
    },
    [showSnackbar, t],
  );

  return {
    isReady: state !== null,
    available: state?.available ?? false,
    allowed: state?.allowed ?? false,
    isSaving,
    setAllowed,
  };
}

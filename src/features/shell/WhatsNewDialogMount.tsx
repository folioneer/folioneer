import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo } from "react";
import { extractSectionFor } from "@/features/whats_new/parseChangelog";
import { useWhatsNewDialog } from "@/features/whats_new/useWhatsNewDialog";
import { WhatsNewDialog } from "@/features/whats_new/WhatsNewDialog";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";
import changelogText from "../../../CHANGELOG.md?raw";

/**
 * Shell-level mount for the What's-new dialog. Two entry paths:
 *
 * - Launch (WNW-020/030): once the app version resolves, compares it against the
 *   last acknowledged version and overlays the pending changelog sections;
 *   dismissing acknowledges the current version (WNW-050).
 * - On demand (WNW-080): `?modal=whats-new` re-opens the current version's
 *   section for re-reading; dismissing only clears the URL param — the
 *   acknowledged version is never touched.
 *
 * The launch dialog takes precedence when both would show, so the
 * acknowledgement path cannot be bypassed by the re-read affordance.
 */
export function WhatsNewDialogMount() {
  const { appVersion, sections, dismiss } = useWhatsNewDialog(changelogText);
  const navigate = useNavigate();
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const modal = typeof search.modal === "string" ? search.modal : undefined;

  useEffect(() => {
    logger.info("[WhatsNewDialogMount] mounted");
  }, []);

  const onDemandSections = useMemo(
    () => (modal === "whats-new" ? extractSectionFor(changelogText, appVersion) : []),
    [modal, appVersion],
  );

  const closeOnDemand = useCallback(() => {
    patchModalSearch(navigate, { modal: undefined }, { replace: true });
  }, [navigate]);

  if (sections) {
    return <WhatsNewDialog version={appVersion} sections={sections} onDismiss={dismiss} />;
  }

  if (onDemandSections.length > 0) {
    return (
      <WhatsNewDialog version={appVersion} sections={onDemandSections} onDismiss={closeOnDemand} />
    );
  }

  return null;
}

import { useTranslation } from "react-i18next";
import { CheckboxField } from "@/ui/components/field/CheckboxField";
import { useAgentsAllowed } from "./useAgentsAllowed";

/**
 * AGT-022 — "Allow agents to connect": off unless the owner switched it on. On a system
 * without an agent connection (AGT-023) it cannot be switched and says so.
 */
export function AgentsAllowedSetting() {
  const { t } = useTranslation();
  const { isReady, available, allowed, isSaving, setAllowed } = useAgentsAllowed();

  if (!isReady) return null;

  return (
    <section id="settings-agents" className="flex flex-col gap-2">
      <CheckboxField
        id="settings-agents-allowed"
        label={t("settings.agents_allowed_label")}
        description={
          available
            ? t("settings.agents_allowed_description")
            : t("settings.agents_unavailable_description")
        }
        checked={allowed}
        onChange={setAllowed}
        disabled={!available || isSaving}
      />
    </section>
  );
}

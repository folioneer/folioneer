import { useEffect, useState } from "react";
import type { Profile } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { profileGateway } from "../gateway";
import { profileErrorToI18n } from "../shared/presenter";

interface UseProfileNameDialogProps {
  isOpen: boolean;
  /** The profile renamed (PFL-030); none creates one (PFL-020). */
  profile: Profile | null;
  onDone: () => void;
}

/**
 * PFL-020 / PFL-030 — the name of a new profile or of one renamed; the core checks it
 * (trimmed, not empty, not another profile's) and its refusal shows under the field.
 */
export function useProfileNameDialog({ isOpen, profile, onDone }: UseProfileNameDialogProps) {
  const [name, setName] = useState("");
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setName(profile?.name ?? "");
      setError(null);
    }
  }, [isOpen, profile]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSubmitting(true);
    const result = profile
      ? await profileGateway.renameProfile(profile.id, name)
      : await profileGateway.createProfile(name);
    setIsSubmitting(false);
    if (result.status === "error") {
      logger.warn("[useProfileNameDialog] the name was refused", { code: result.error.code });
      setError(profileErrorToI18n(result.error));
      return;
    }
    onDone();
  };

  return { name, setName, error, isSubmitting, submit };
}

import { useEffect, useState } from "react";
import type { ImpliedCostBasis } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountGateway } from "../gateway";
import type { AccountFormData } from "../shared/AccountForm";
import { useProfileChoices } from "../shared/useProfileChoices";
import { validateAccountCurrency, validateAccountName } from "../shared/validateAccount";
import { useAccounts } from "../useAccounts";

interface UseAddAccountProps {
  onSubmitSuccess?: () => void;
  /** PFL-011 — the default profile's name in the window's language, for the holder's prefill. */
  defaultProfileName?: string;
}

export function useAddAccount({
  onSubmitSuccess,
  defaultProfileName = "",
}: UseAddAccountProps = {}) {
  const { addAccount } = useAccounts();
  // FXR-128 — a new account's currency starts as the reference currency.
  const referenceCurrency = useAppStore((s) => s.referenceCurrency);

  const [formData, setFormData] = useState<AccountFormData>({
    name: "",
    bank_name: "",
    currency: referenceCurrency,
    management_fees_enabled: false,
    country: "",
    wrapper: "",
    cost_basis_method: "",
    profile_id: "",
    holder: "",
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // PFL-021 — a new account starts in the profile in use, once the profiles are read.
  const { list: profiles, error: profilesError } = useProfileChoices();

  const inUse = profiles?.in_use ?? "";
  // #769 (the validated mock) — a new account's holder starts as the profile in use's name.
  const inUseName =
    profiles?.profiles.find((summary) => summary.profile.id === inUse)?.profile.name ??
    defaultProfileName;
  useEffect(() => {
    if (inUse)
      setFormData((prev) =>
        prev.profile_id ? prev : { ...prev, profile_id: inUse, holder: prev.holder || inUseName },
      );
  }, [inUse, inUseName]);
  // CBM-014 — the method the country and wrapper imply, read again when either changes.
  const [impliedCostBasis, setImpliedCostBasis] = useState<ImpliedCostBasis | null>(null);
  const { country, wrapper } = formData;
  useEffect(() => {
    let current = true;
    accountGateway
      .getImpliedCostBasis(country || null, wrapper || null)
      .then((implied) => {
        if (current) setImpliedCostBasis(implied);
      })
      .catch((err) => {
        logger.warn("[useAddAccount] the implied method could not be read", { err });
        if (current) setImpliedCostBasis(null);
      });
    return () => {
      current = false;
    };
  }, [country, wrapper]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    const { name, value, type } = e.target;
    setFormData((prev) => ({
      ...prev,
      [name]: type === "checkbox" && "checked" in e.target ? e.target.checked : value,
    }));
  };

  // ACC-043 — a change of country keeps the wrapper: any account may hold any wrapper.
  const setCountry = (country: string) => setFormData((prev) => ({ ...prev, country }));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // R14 — block if name is empty or whitespace-only
    const validationError =
      validateAccountName(formData.name) ?? validateAccountCurrency(formData.currency);
    if (validationError) {
      setError(validationError);
      return;
    }

    setError(null);
    setIsSubmitting(true);

    const result = await addAccount({
      name: formData.name,
      bank_name: formData.bank_name,
      currency: formData.currency,
      management_fees_enabled: formData.management_fees_enabled,
      country: formData.country || null,
      wrapper: formData.wrapper || null,
      cost_basis_method: formData.cost_basis_method || null,
      profile_id: formData.profile_id || null,
      holder: formData.holder || null,
    });

    setIsSubmitting(false);

    // R13 — keep modal open on error
    if (result.error) {
      setError(result.error);
      return;
    }

    setFormData({
      name: "",
      bank_name: "",
      currency: referenceCurrency,
      management_fees_enabled: false,
      country: "",
      wrapper: "",
      cost_basis_method: "",
      profile_id: profiles?.in_use ?? "",
      holder: inUseName,
    });

    if (onSubmitSuccess) {
      onSubmitSuccess();
    }
  };

  return {
    formData,
    // F27 — the profiles that cannot be read show as the form's error.
    error: error ?? profilesError,
    isSubmitting,
    handleChange,
    setCountry,
    handleSubmit,
    impliedCostBasis,
    profiles: profiles?.profiles ?? null,
  };
}

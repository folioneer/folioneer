import { useState } from "react";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";
import type { AccountFormData } from "../shared/AccountForm";
import { validateAccountCurrency, validateAccountName } from "../shared/validateAccount";
import { useAccounts } from "../useAccounts";

interface UseAddAccountProps {
  onSubmitSuccess?: () => void;
}

export function useAddAccount({ onSubmitSuccess }: UseAddAccountProps = {}) {
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
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

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
    });

    if (onSubmitSuccess) {
      onSubmitSuccess();
    }
  };

  return {
    formData,
    error,
    isSubmitting,
    handleChange,
    setCountry,
    handleSubmit,
  };
}

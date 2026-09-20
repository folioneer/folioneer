import { useEffect, useState } from "react";
import type { Account } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import type { AccountFormData } from "../shared/AccountForm";
import { FREQUENCIES } from "../shared/presenter";
import { validateAccountCurrency, validateAccountName } from "../shared/validateAccount";
import { useAccounts } from "../useAccounts";

interface UseEditAccountModalProps {
  account: Account | null;
  onClose: () => void;
}

export function useEditAccountModal({ account, onClose }: UseEditAccountModalProps) {
  const { updateAccount } = useAccounts();

  const [formData, setFormData] = useState<AccountFormData>({
    name: "",
    bank_name: "",
    currency: "EUR",
    update_frequency: "ManualMonth",
    management_fees_enabled: false,
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Sync form data when account changes; reset error (R13)
  useEffect(() => {
    if (account) {
      setFormData({
        name: account.name,
        bank_name: account.bank_name,
        currency: account.currency,
        update_frequency: account.update_frequency,
        management_fees_enabled: account.management_fees_enabled,
      });
      setError(null);
    }
  }, [account]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    const { name, value, type } = e.target;
    setFormData((prev) => ({
      ...prev,
      [name]: type === "checkbox" && "checked" in e.target ? e.target.checked : value,
    }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!account) return;

    const validationError =
      validateAccountName(formData.name) ?? validateAccountCurrency(formData.currency);
    if (validationError) {
      setError(validationError);
      return;
    }

    setError(null);
    setIsSubmitting(true);

    const result = await updateAccount({
      id: account.id,
      name: formData.name,
      bank_name: formData.bank_name,
      currency: formData.currency,
      update_frequency: formData.update_frequency,
      management_fees_enabled: formData.management_fees_enabled,
    });

    setIsSubmitting(false);

    // R13, R15 — keep modal open on error
    if (result.error) {
      setError(result.error);
      return;
    }

    onClose();
  };

  return {
    formData,
    error,
    isSubmitting,
    handleChange,
    handleSubmit,
    frequencies: FREQUENCIES,
  };
}

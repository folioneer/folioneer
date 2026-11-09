import { useEffect, useState } from "react";
import type { Account } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountGateway } from "../gateway";
import type { AccountFormData } from "../shared/AccountForm";
import { accountMutationErrorToI18n } from "../shared/presenter";
import { useProfileChoices } from "../shared/useProfileChoices";
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
    management_fees_enabled: false,
    country: "",
    wrapper: "",
    cost_basis_method: "",
    profile_id: "",
    holder: "",
  });
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // #515 (the validated mock) — the method differs from the account's: its gains recompute.
  const costBasisChanged =
    account !== null &&
    formData.cost_basis_method !== "" &&
    formData.cost_basis_method !== (account.cost_basis_method ?? "WeightedAverage");
  // PFL-031 — the profiles to choose from; a different one moves the account with its history.
  const { list: profiles, error: profilesError } = useProfileChoices();

  const profileChanged =
    account !== null && formData.profile_id !== "" && formData.profile_id !== account.profile_id;
  const profileOf = (id: string | undefined) =>
    profiles?.profiles.find((summary) => summary.profile.id === id)?.profile ?? null;
  const profileMove = profileChanged
    ? { from: profileOf(account?.profile_id), to: profileOf(formData.profile_id) }
    : null;

  // Sync form data when account changes; reset error (R13)
  useEffect(() => {
    if (account) {
      setFormData({
        name: account.name,
        bank_name: account.bank_name,
        currency: account.currency,
        management_fees_enabled: account.management_fees_enabled,
        country: account.country ?? "",
        wrapper: account.wrapper ?? "",
        cost_basis_method: account.cost_basis_method ?? "WeightedAverage",
        profile_id: account.profile_id ?? "",
        holder: account.holder ?? "",
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

  // ACC-043 — a change of country keeps the wrapper: any account may hold any wrapper.
  const setCountry = (country: string) => setFormData((prev) => ({ ...prev, country }));

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

    // PFL-031 — a move is its own action, made first: the account and its whole history go to the
    // profile, and a refused move leaves every other field unsaved.
    if (profileChanged) {
      setIsSubmitting(true);
      const moved = await accountGateway.moveAccountToProfile(account.id, formData.profile_id);
      setIsSubmitting(false);
      if (moved.status === "error") {
        logger.warn("[useEditAccountModal] the account was not moved", { code: moved.error.code });
        setError(accountMutationErrorToI18n(moved.error));
        return;
      }
    }

    setIsSubmitting(true);

    const result = await updateAccount({
      id: account.id,
      name: formData.name,
      bank_name: formData.bank_name,
      currency: formData.currency,
      management_fees_enabled: formData.management_fees_enabled,
      country: formData.country || null,
      wrapper: formData.wrapper || null,
    });

    setIsSubmitting(false);

    // R13, R15 — keep modal open on error
    if (result.error) {
      setError(result.error);
      return;
    }

    // ACC-050 — a changed holder is its own action; no figure moves.
    if (formData.holder.trim() !== (account.holder ?? "")) {
      setIsSubmitting(true);
      const named = await accountGateway.setAccountHolder(account.id, formData.holder || null);
      setIsSubmitting(false);
      if (named.status === "error") {
        logger.warn("[useEditAccountModal] the holder was not set", { code: named.error.code });
        setError(accountMutationErrorToI18n(named.error));
        return;
      }
    }

    // CBM-030 — a changed method is its own action: the account's history recomputes.
    if (costBasisChanged && formData.cost_basis_method !== "") {
      setIsSubmitting(true);
      try {
        const changed = await accountGateway.setAccountCostBasisMethod(
          account.id,
          formData.cost_basis_method,
        );
        if (changed.status === "error") {
          logger.warn("[useEditAccountModal] the method was not changed", {
            code: changed.error.code,
          });
          setError({ key: "account.form_cost_basis_change_failed" });
          return;
        }
      } catch (err) {
        logger.error("[useEditAccountModal] the change of method threw", { err });
        setError({ key: "account.form_cost_basis_change_failed" });
        return;
      } finally {
        setIsSubmitting(false);
      }
    }

    onClose();
  };

  return {
    formData,
    // F27 — the profiles that cannot be read show as the form's error.
    error: error ?? profilesError,
    isSubmitting,
    handleChange,
    setCountry,
    handleSubmit,
    costBasisChanged,
    profiles: profiles?.profiles ?? null,
    profileMove,
  };
}

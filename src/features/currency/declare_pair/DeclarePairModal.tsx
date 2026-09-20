import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { Dialog } from "@/ui/components/modal/Dialog";
import { useDeclarePair } from "./useDeclarePair";

interface DeclarePairModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

/** FXR-054/055 — modal to declare a directed currency pair the system follows. */
export function DeclarePairModal({ isOpen, onClose, onSuccess }: DeclarePairModalProps) {
  const { t } = useTranslation();
  const {
    fromCurrency,
    toCurrency,
    setFromCurrency,
    setToCurrency,
    isSubmitting,
    error,
    isSubmitDisabled,
    submit,
  } = useDeclarePair({ onSuccess });

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      <Button
        type="button"
        variant="primary"
        id="declare-pair-submit"
        data-testid="declare-pair-submit"
        loading={isSubmitting}
        disabled={isSubmitDisabled || isSubmitting}
        onClick={() => void submit()}
      >
        {t("action.add")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="declare-pair-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("currency.declare_pair_title")}
      actions={actions}
    >
      <div className="flex flex-col gap-4 py-2">
        <TextField
          id="declare-pair-from"
          data-testid="declare-pair-from"
          label={t("currency.form_from_label")}
          value={fromCurrency}
          maxLength={3}
          autoCapitalize="characters"
          onChange={(e) => setFromCurrency(e.target.value.toUpperCase())}
        />
        <TextField
          id="declare-pair-to"
          data-testid="declare-pair-to"
          label={t("currency.form_to_label")}
          value={toCurrency}
          maxLength={3}
          autoCapitalize="characters"
          onChange={(e) => setToCurrency(e.target.value.toUpperCase())}
        />
        {error && (
          <p role="alert" data-testid="declare-pair-error" className="text-sm text-m3-error">
            {t(error.key, error.vars)}
          </p>
        )}
      </div>
    </Dialog>
  );
}

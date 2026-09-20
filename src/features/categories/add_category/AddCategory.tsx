import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { CategoryForm } from "../shared/CategoryForm";
import { useAddCategory } from "./useAddCategory";

interface AddCategoryModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function AddCategoryModal({ isOpen, onClose }: AddCategoryModalProps) {
  const { t } = useTranslation();
  const { name, error, isSubmitting, handleChange, handleSubmit } = useAddCategory({
    onSubmitSuccess: onClose,
  });

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="add-category-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || !name.trim()}
      >
        {t("action.add")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="add-category-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("category.add_modal_title")}
      actions={actions}
      maxWidth="max-w-xl"
    >
      <form id="add-category-form" className="py-2" onSubmit={handleSubmit}>
        <CategoryForm name={name} handleChange={handleChange} idPrefix="add-category" />
        {error && <p className="mt-3 text-sm text-m3-error">{t(error.key, error.vars)}</p>}
      </form>
    </Dialog>
  );
}

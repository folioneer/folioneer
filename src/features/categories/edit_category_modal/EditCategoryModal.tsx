import { useTranslation } from "react-i18next";
import type { AssetCategory } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { Dialog } from "@/ui/components/modal/Dialog";
import { CategoryForm } from "../shared/CategoryForm";
import { useEditCategoryModal } from "./useEditCategoryModal";

interface EditCategoryModalProps {
  isOpen: boolean;
  onClose: () => void;
  category: AssetCategory | null;
}

export function EditCategoryModal({ isOpen, onClose, category }: EditCategoryModalProps) {
  const { t } = useTranslation();
  const { name, error, isSubmitting, handleChange, handleSubmit } = useEditCategoryModal({
    category,
    onClose,
  });

  const actions = (
    <>
      <Button variant="secondary" onClick={onClose} disabled={isSubmitting}>
        {t("action.cancel")}
      </Button>
      <Button
        type="submit"
        form="edit-category-form"
        variant="primary"
        loading={isSubmitting}
        disabled={isSubmitting || !name.trim() || name.trim() === category?.name}
      >
        {t("action.save")}
      </Button>
    </>
  );

  return (
    <Dialog
      id="edit-category-dialog"
      isOpen={isOpen}
      onClose={onClose}
      title={t("category.edit_modal_title")}
      actions={actions}
      maxWidth="max-w-xl"
    >
      <form id="edit-category-form" className="py-2" onSubmit={handleSubmit}>
        <CategoryForm name={name} handleChange={handleChange} idPrefix="edit-category" />
        {error && <p className="mt-3 text-sm text-m3-error">{t(error.key, error.vars)}</p>}
      </form>
    </Dialog>
  );
}

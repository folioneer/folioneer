import { useEffect, useState } from "react";
import type { AssetCategory } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { useCategories } from "../useCategories";

interface UseEditCategoryModalProps {
  category: AssetCategory | null;
  onClose: () => void;
}

export function useEditCategoryModal({ category, onClose }: UseEditCategoryModalProps) {
  const { updateCategory } = useCategories();
  const [name, setName] = useState("");
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (category) {
      setName(category.name);
      setError(null);
    }
  }, [category]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
    setError(null);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!category || !name.trim()) return;

    setIsSubmitting(true);
    const result = await updateCategory(category.id, name.trim());
    if (result.error) {
      setError(result.error);
    } else {
      onClose();
    }
    setIsSubmitting(false);
  };

  return { name, error, isSubmitting, handleChange, handleSubmit };
}

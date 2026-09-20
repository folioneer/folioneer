import { useState } from "react";
import type { I18nMessage } from "@/ui/format/i18n";
import { useCategories } from "../useCategories";

interface UseAddCategoryProps {
  onSubmitSuccess?: () => void;
}

export function useAddCategory({ onSubmitSuccess }: UseAddCategoryProps = {}) {
  const { addCategory } = useCategories();
  const [name, setName] = useState("");
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
    setError(null);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    setIsSubmitting(true);
    const result = await addCategory(name.trim());
    if (result.error) {
      setError(result.error);
    } else {
      setName("");
      setError(null);
      onSubmitSuccess?.();
    }
    setIsSubmitting(false);
  };

  return { name, error, isSubmitting, handleChange, handleSubmit };
}

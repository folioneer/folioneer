import { useTranslation } from "react-i18next";
import { TextField } from "@/ui/components/field/TextField";

interface CategoryFormProps {
  name: string;
  handleChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  idPrefix?: string;
}

export function CategoryForm({ name, handleChange, idPrefix = "category" }: CategoryFormProps) {
  const { t } = useTranslation();

  return (
    <TextField
      label={t("category.form_name_label")}
      id={`${idPrefix}-name`}
      name="name"
      required
      placeholder={t("category.form_name_placeholder")}
      value={name}
      onChange={handleChange}
    />
  );
}

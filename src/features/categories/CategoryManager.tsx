import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FAB } from "@/ui/components/fab/FAB";
import { ManagerLayout } from "@/ui/components/layout/ManagerLayout";
import { AddCategoryModal } from "./add_category/AddCategory";
import { CategoryTable } from "./category_table/CategoryTable";

export function CategoryManager() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);

  return (
    <>
      <ManagerLayout
        searchId="category-search"
        searchTerm={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("category.search_placeholder")}
        table={<CategoryTable searchTerm={query} />}
      />
      <FAB onClick={() => setIsAddModalOpen(true)} label={t("category.fab_label")} />
      <AddCategoryModal isOpen={isAddModalOpen} onClose={() => setIsAddModalOpen(false)} />
    </>
  );
}

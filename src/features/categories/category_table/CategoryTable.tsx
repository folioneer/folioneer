import { ArrowDown, ArrowUp, Edit2, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AssetCategory } from "@/bindings";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { Dialog } from "@/ui/components/modal/Dialog";
import type { I18nMessage } from "@/ui/format/i18n";
import { EditCategoryModal } from "../edit_category_modal/EditCategoryModal";
import { isSystemCategory } from "../shared/presenter";
import { useCategories } from "../useCategories";
import { useCategoryTable } from "./useCategoryTable";

interface CategoryTableProps {
  searchTerm: string;
}

export function CategoryTable({ searchTerm }: CategoryTableProps) {
  const { t } = useTranslation();
  const {
    categories,
    loading,
    error: loadError,
    fetchCategories,
    deleteCategory,
  } = useCategories();
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(null);

  const { sortedAndFilteredCategories, sortConfig, handleSort } = useCategoryTable(
    categories,
    searchTerm,
  );

  const [deleteData, setDeleteData] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [deleteError, setDeleteError] = useState<I18nMessage | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [editData, setEditData] = useState<AssetCategory | null>(null);

  useEffect(() => {
    logger.info("[CategoryTable] mounted");
  }, []);

  const handleDeleteConfirm = async () => {
    if (!deleteData) return;
    setIsDeleting(true);
    const result = await deleteCategory(deleteData.id);
    if (result.error) {
      setDeleteError(result.error);
    } else {
      setDeleteData(null);
      setDeleteError(null);
    }
    setIsDeleting(false);
  };

  return (
    <div className="m3-table-container flex-1">
      <table className="w-full border-collapse">
        <thead className="sticky top-0 bg-m3-surface-container z-10">
          <tr>
            <th className="m3-th cursor-pointer" onClick={() => handleSort("name")}>
              <div className="flex items-center">
                {t("category.column_name")}
                {sortConfig.key === "name" &&
                  (sortConfig.direction === "asc" ? (
                    <ArrowUp size={14} className="ml-1 text-m3-primary" />
                  ) : (
                    <ArrowDown size={14} className="ml-1 text-m3-primary" />
                  ))}
              </div>
            </th>
            <th className="m3-th text-right">{t("category.column_actions")}</th>
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <tr>
              <td colSpan={2} className="m3-td text-center py-12">
                <span className="text-m3-on-surface-variant animate-pulse">
                  {t("category.loading")}
                </span>
              </td>
            </tr>
          ) : categories.length === 0 ? (
            <tr>
              <td colSpan={2} className="m3-td text-center py-12 text-m3-on-surface-variant">
                {t("category.empty")}
              </td>
            </tr>
          ) : loadError ? (
            <tr>
              <td colSpan={2} className="m3-td text-center py-12">
                <div className="flex flex-col items-center gap-3">
                  <p role="alert" className="text-sm text-m3-error">
                    {t("category.error_load")}
                  </p>
                  <Button variant="outline" size="sm" onClick={fetchCategories}>
                    {t("action.retry")}
                  </Button>
                </div>
              </td>
            </tr>
          ) : sortedAndFilteredCategories.length === 0 ? (
            <tr>
              <td colSpan={2} className="m3-td text-center py-12 text-m3-on-surface-variant">
                {t("category.empty")}
              </td>
            </tr>
          ) : (
            sortedAndFilteredCategories.map((category: AssetCategory) => (
              <tr
                key={category.id}
                onClick={() => setSelectedCategoryId(category.id)}
                className={`m3-tr ${selectedCategoryId === category.id ? "m3-tr-selected" : ""}`}
              >
                <td className="m3-td font-medium text-m3-on-surface">
                  <div className="flex items-center gap-2">
                    {category.name}
                    {isSystemCategory(category.id) && (
                      <span className="px-2 py-0.5 text-xs rounded-full bg-m3-secondary-container text-m3-on-secondary-container font-medium">
                        {t("category.badge_default")}
                      </span>
                    )}
                  </div>
                </td>
                <td className="m3-td text-right">
                  <div className="flex items-center justify-end gap-1">
                    <IconButton
                      icon={<Edit2 size={16} />}
                      variant="ghost"
                      shape="square"
                      size="sm"
                      aria-label={t("action.edit")}
                      disabled={isSystemCategory(category.id)}
                      onClick={(e) => {
                        e.stopPropagation();
                        if (!isSystemCategory(category.id)) setEditData(category);
                      }}
                    />
                    <IconButton
                      icon={<Trash2 size={16} />}
                      variant="danger"
                      shape="square"
                      size="sm"
                      aria-label={t("action.delete")}
                      disabled={isSystemCategory(category.id)}
                      onClick={(e) => {
                        e.stopPropagation();
                        if (!isSystemCategory(category.id)) {
                          setDeleteError(null);
                          setDeleteData({
                            id: category.id,
                            name: category.name,
                          });
                        }
                      }}
                    />
                  </div>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      <EditCategoryModal
        isOpen={!!editData}
        onClose={() => setEditData(null)}
        category={editData}
      />

      <Dialog
        isOpen={!!deleteData}
        onClose={() => {
          if (!isDeleting) {
            setDeleteData(null);
            setDeleteError(null);
          }
        }}
        title={t("category.delete_confirm_title")}
        disableClose={isDeleting}
        actions={
          <div className="flex items-center justify-end gap-3">
            <Button
              variant="ghost"
              onClick={() => {
                setDeleteData(null);
                setDeleteError(null);
              }}
              disabled={isDeleting}
            >
              {t("action.cancel")}
            </Button>
            <Button
              variant="danger"
              onClick={handleDeleteConfirm}
              loading={isDeleting}
              disabled={isDeleting}
            >
              {t("action.confirm")}
            </Button>
          </div>
        }
      >
        <p className="text-m3-on-surface-variant leading-relaxed">
          {deleteError
            ? t(deleteError.key, deleteError.vars)
            : t("category.delete_confirm_message")}
        </p>
      </Dialog>
    </div>
  );
}

import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { FAB } from "@/ui/components/fab/FAB";
import { ManagerLayout } from "@/ui/components/layout/ManagerLayout";
import { AssetTable } from "./asset_table/AssetTable";
import { WebLookupModal } from "./web_lookup";

type ReturnNavTarget =
  | { type: "txList"; accountId: string; assetId: string }
  | { type: "account"; accountId: string }
  | { type: "addTransaction"; prefillAccountId?: string }
  | { type: "assets" };

function resolveReturnNav(returnPath: string | undefined): ReturnNavTarget {
  if (!returnPath) return { type: "assets" };
  const txMatch = returnPath.match(/^\/accounts\/([^/]+)\/transactions\/([^/]+)$/);
  // biome-ignore lint/style/noNonNullAssertion: groups are present when regex matches
  if (txMatch) return { type: "txList", accountId: txMatch[1]!, assetId: txMatch[2]! };
  const accMatch = returnPath.match(/^\/accounts\/([^/]+)$/);
  // biome-ignore lint/style/noNonNullAssertion: group is present when regex matches
  if (accMatch) return { type: "account", accountId: accMatch[1]! };
  const txNewMatch = returnPath.match(/^\/transactions\/new(?:\?prefillAccountId=([^&]+))?$/);
  if (txNewMatch) return { type: "addTransaction", prefillAccountId: txNewMatch[1] };
  return { type: "assets" };
}

function ArchivedToggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <label
      data-testid="show-archived-toggle"
      className="flex items-center gap-2 text-sm text-m3-on-surface-variant cursor-pointer select-none shrink-0"
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-m3-primary"
      />
      {t("asset.toggle_show_archived")}
    </label>
  );
}

export function AssetManager() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { createNew, returnPath } = useSearch({ from: "/assets" });

  const [query, setQuery] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [isAddModalOpen, setIsAddModalOpen] = useState(!!createNew);

  useEffect(() => {
    logger.info("[AssetManager] mounted");
  }, []);

  const handleAddAssetSuccess = useCallback(
    (newAssetId: string) => {
      setIsAddModalOpen(false);
      const target = resolveReturnNav(returnPath);
      if (target.type === "txList") {
        navigate({
          to: "/accounts/$accountId/transactions/$assetId",
          params: { accountId: target.accountId, assetId: target.assetId },
          search: { pendingTransactionAssetId: newAssetId },
        });
      } else if (target.type === "account") {
        navigate({
          to: "/accounts/$accountId",
          params: { accountId: target.accountId },
        });
      } else if (target.type === "addTransaction") {
        navigate({
          to: "/transactions/new",
          search: {
            prefillAssetId: newAssetId,
            prefillAccountId: target.prefillAccountId,
          },
        });
      } else {
        navigate({
          to: "/assets",
          search: { createNew: undefined, returnPath: undefined },
        });
      }
    },
    [navigate, returnPath],
  );

  const handleOpenAddModal = useCallback(() => setIsAddModalOpen(true), []);

  return (
    <>
      <ManagerLayout
        searchId="asset-search"
        searchTerm={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("asset.search_placeholder")}
        searchExtra={<ArchivedToggle checked={showArchived} onChange={setShowArchived} />}
        table={<AssetTable searchTerm={query} showArchived={showArchived} />}
      />
      <FAB id="fab-add-asset" onClick={handleOpenAddModal} label={t("asset.fab_label")} />
      {isAddModalOpen && (
        <WebLookupModal
          isOpen={true}
          prefillName={createNew}
          onSuccess={handleAddAssetSuccess}
          onClose={() => setIsAddModalOpen(false)}
        />
      )}
    </>
  );
}

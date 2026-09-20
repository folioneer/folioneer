import { useNavigate } from "@tanstack/react-router";
import { TrendingUp } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { selectUnpricedModalOpen, useAppStore } from "@/lib/store";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { FAB } from "@/ui/components/fab/FAB";
import { ManagerLayout } from "@/ui/components/layout/ManagerLayout";
import { AccountTable } from "./account_table/AccountTable";
import { AddAccountModal } from "./add_account/AddAccountModal";
import { PriceMovementDialog } from "./price_movement/PriceMovementDialog";
import { usePriceMovementReport } from "./price_movement/usePriceMovementReport";
import { useRefreshGlobalPrices } from "./refresh_prices/useRefreshGlobalPrices";

export function AccountManager() {
  // PMV-013/016 — the report belongs to this mounted surface; navigating away discards it.
  const priceMovement = usePriceMovementReport();
  // PMV-018 — the same refresh can leave assets unpriced, which auto-opens the
  // manual-fill modal (MKT-172). That modal asks the user for something; this
  // dialog only tells them something, so it waits its turn rather than
  // stacking over it.
  const isUnpricedModalOpen = useAppStore(selectUnpricedModalOpen);
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const navigate = useNavigate();
  const { isPending: isRefreshPending, refresh: refreshPrices } = useRefreshGlobalPrices();
  const handleAccountClick = useCallback(
    (id: string) => navigate({ to: "/accounts/$accountId", params: { accountId: id } }),
    [navigate],
  );

  useEffect(() => {
    logger.info("[AccountManager] mounted");
  }, []);

  return (
    <>
      <ManagerLayout
        searchId="account-search"
        searchTerm={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("account.search_placeholder")}
        searchExtra={
          <>
            {/* GPF — entry point to the portfolio-wide performance page */}
            <IconButton
              id="accounts-performance"
              shape="square"
              size="sm"
              variant="tonal"
              icon={<TrendingUp size={16} />}
              onClick={() => void navigate({ to: "/performance" })}
              aria-label={t("account.action_global_performance")}
              title={t("account.action_global_performance")}
            />
            <Button
              id="account-manager-refresh-prices"
              variant="tonal"
              size="sm"
              loading={isRefreshPending}
              onClick={() => void refreshPrices()}
              aria-label={t("account.refresh_prices")}
            >
              {t("account.refresh_prices")}
            </Button>
          </>
        }
        table={<AccountTable searchTerm={query} onAccountClick={handleAccountClick} />}
      />
      {/* R14 — FAB opens add modal */}
      <FAB
        id="fab-add-account"
        onClick={() => setIsAddModalOpen(true)}
        label={t("account.fab_label")}
      />
      <AddAccountModal isOpen={isAddModalOpen} onClose={() => setIsAddModalOpen(false)} />
      {priceMovement.report !== null && (
        <PriceMovementDialog
          report={priceMovement.report}
          isOpen={!isUnpricedModalOpen}
          onDismiss={priceMovement.dismiss}
        />
      )}
    </>
  );
}

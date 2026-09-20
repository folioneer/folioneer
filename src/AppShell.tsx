import { Outlet, useRouterState } from "@tanstack/react-router";
import { useEffect } from "react";
import { AddTransactionModalMount } from "@/features/shell/AddTransactionModalMount";
import { AssetEditModalMount } from "@/features/shell/AssetEditModalMount";
import { CashTransactionEditMount } from "@/features/shell/CashTransactionEditMount";
import { CurrencyRateEditMount } from "@/features/shell/CurrencyRateEditMount";
import { FreeSharesEditModalMount } from "@/features/shell/FreeSharesEditModalMount";
import { InterestEditModalMount } from "@/features/shell/InterestEditModalMount";
import { MainLayout } from "@/features/shell/MainLayout";
import { ManagementFeeEditModalMount } from "@/features/shell/ManagementFeeEditModalMount";
import { SplitEditModalMount } from "@/features/shell/SplitEditModalMount";
import { UnpricedPricesModalMount } from "@/features/shell/UnpricedPricesModalMount";
import { WhatsNewDialogMount } from "@/features/shell/WhatsNewDialogMount";
import { saveLastPath } from "@/lib/lastPath";

export function AppShell() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  useEffect(() => {
    saveLastPath(pathname);
  }, [pathname]);

  return (
    <MainLayout>
      <Outlet />
      <AssetEditModalMount />
      <CashTransactionEditMount />
      <CurrencyRateEditMount />
      <FreeSharesEditModalMount />
      <InterestEditModalMount />
      <ManagementFeeEditModalMount />
      <SplitEditModalMount />
      <UnpricedPricesModalMount />
      <AddTransactionModalMount />
      <WhatsNewDialogMount />
    </MainLayout>
  );
}

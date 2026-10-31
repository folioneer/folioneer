import { Outlet, useRouterState } from "@tanstack/react-router";
import { useEffect } from "react";
import { AddTransactionModalMount } from "@/features/shell/AddTransactionModalMount";
import { AssetCreationMount } from "@/features/shell/AssetCreationMount";
import { AssetEditModalMount } from "@/features/shell/AssetEditModalMount";
import { AgentConnectionMount } from "@/features/shell/agent_connection/AgentConnectionMount";
import { CashTransactionEditMount } from "@/features/shell/CashTransactionEditMount";
import { CurrencyRateEditMount } from "@/features/shell/CurrencyRateEditMount";
import { DividendEditModalMount } from "@/features/shell/DividendEditModalMount";
import { FreeSharesEditModalMount } from "@/features/shell/FreeSharesEditModalMount";
import { InterestEditModalMount } from "@/features/shell/InterestEditModalMount";
import { MainLayout } from "@/features/shell/MainLayout";
import { ManagementFeeEditModalMount } from "@/features/shell/ManagementFeeEditModalMount";
import { SplitEditModalMount } from "@/features/shell/SplitEditModalMount";
import { TransferEditModalMount } from "@/features/shell/TransferEditModalMount";
import { UnpricedPricesModalMount } from "@/features/shell/UnpricedPricesModalMount";
import { WhatsNewDialogMount } from "@/features/shell/WhatsNewDialogMount";
import { WriteOffEditModalMount } from "@/features/shell/WriteOffEditModalMount";
import { saveLastPath } from "@/lib/lastPath";

export function AppShell() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  useEffect(() => {
    saveLastPath(pathname);
  }, [pathname]);

  return (
    <MainLayout>
      <Outlet />
      <AssetCreationMount />
      <AssetEditModalMount />
      <CashTransactionEditMount />
      <CurrencyRateEditMount />
      <DividendEditModalMount />
      <FreeSharesEditModalMount />
      <InterestEditModalMount />
      <ManagementFeeEditModalMount />
      <SplitEditModalMount />
      <WriteOffEditModalMount />
      <TransferEditModalMount />
      <UnpricedPricesModalMount />
      <AddTransactionModalMount />
      <WhatsNewDialogMount />
      <AgentConnectionMount />
    </MainLayout>
  );
}

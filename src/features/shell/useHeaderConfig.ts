import { useNavigate, useRouterState } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { useAppStore } from "@/lib/store";
import { NAV_ITEMS } from "./navItems";

interface HeaderConfig {
  title: string;
  onBack?: () => void;
}

export function useHeaderConfig(): HeaderConfig {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useRouterState({ select: (s) => s.location });
  const accounts = useAppStore((s) => s.accounts);

  const { pathname, searchStr } = location;

  // /accounts/$accountId/transactions/$assetId
  const txAccountId = pathname.match(/^\/accounts\/([^/]+)\/transactions\/([^/]+)$/)?.[1];
  if (txAccountId) {
    return {
      title: t("transaction.list_title"),
      onBack: () => navigate({ to: "/accounts/$accountId", params: { accountId: txAccountId } }),
    };
  }

  // /accounts/$accountId
  const accountId = pathname.match(/^\/accounts\/([^/]+)$/)?.[1];
  if (accountId) {
    const account = accounts.find((a) => a.id === accountId);
    return {
      title: account?.name ?? t("account_details.title"),
      onBack: () => navigate({ to: "/accounts" }),
    };
  }

  // /transactions/new
  if (pathname === "/transactions/new") {
    const params = new URLSearchParams(searchStr);
    const prefillAccountId = params.get("prefillAccountId") ?? undefined;
    return {
      title: t("transaction.add_modal_title"),
      onBack: () => {
        if (prefillAccountId) {
          navigate({
            to: "/accounts/$accountId",
            params: { accountId: prefillAccountId },
          });
        } else {
          navigate({
            to: "/assets",
            search: { createNew: undefined, returnPath: undefined },
          });
        }
      },
    };
  }

  // Top-level nav items
  const exact = NAV_ITEMS.find((item) => item.path === pathname);
  if (exact) return { title: t(exact.labelKey) };
  const parent = NAV_ITEMS.find(
    (item) => item.path !== "/" && pathname.startsWith(`${item.path}/`),
  );
  return { title: parent ? t(parent.labelKey) : "" };
}

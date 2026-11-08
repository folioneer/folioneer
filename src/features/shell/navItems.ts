import { ArrowLeftRight, Palette, PieChart, Tag, Users, Wallet } from "lucide-react";
import type { ElementType } from "react";

export interface NavItem {
  labelKey: string;
  path: string;
  icon: ElementType;
}

const BASE_NAV_ITEMS: NavItem[] = [
  { labelKey: "nav.accounts", path: "/accounts", icon: Wallet },
  { labelKey: "nav.assets", path: "/assets", icon: PieChart },
  { labelKey: "nav.categories", path: "/categories", icon: Tag },
  { labelKey: "nav.currency_rates", path: "/currency-rates", icon: ArrowLeftRight },
];

const DEV_NAV_ITEMS: NavItem[] = import.meta.env.DEV
  ? [{ labelKey: "nav.design_system", path: "/design-system", icon: Palette }]
  : [];

export const NAV_ITEMS: NavItem[] = [...BASE_NAV_ITEMS, ...DEV_NAV_ITEMS];

/** #768 (the validated mock) — the profiles overview, first, shown from two profiles on. */
export const PROFILES_NAV_ITEM: NavItem = {
  labelKey: "nav.profiles",
  path: "/profiles",
  icon: Users,
};

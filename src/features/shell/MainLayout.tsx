import { useEffect, useState } from "react";
import { UpdateBanner, useUpdateBanner } from "@/features/update";
import { logger } from "@/lib/logger";
import { Snackbar } from "@/ui/components";
import { Content } from "./Content";
import { useFeeGeneration } from "./fee_generation/useFeeGeneration";
import { Header } from "./Header";
import { PriceFetchProgressBar } from "./PriceFetchProgressBar";
import { Sidebar } from "./Sidebar";

interface MainLayoutProps {
  children: React.ReactNode;
}

export function MainLayout({ children }: MainLayoutProps) {
  const [isDrawerOpen, setDrawerOpen] = useState(() => {
    const saved = localStorage.getItem("drawer_open");
    return saved === null ? true : saved === "true";
  });

  useEffect(() => {
    logger.info("[MainLayout] mounted");
  }, []);

  useEffect(() => {
    localStorage.setItem("drawer_open", isDrawerOpen.toString());
  }, [isDrawerOpen]);

  // R4 — banner is part of permanent shell layout, visible on all pages
  const updateBannerData = useUpdateBanner();

  // FEE-040 — apply due recurring management-fee deductions once on app start.
  useFeeGeneration();

  return (
    <div className="flex h-screen overflow-hidden bg-m3-surface">
      <Sidebar isOpen={isDrawerOpen} toggleDrawer={() => setDrawerOpen(!isDrawerOpen)} />

      {/* Main Container */}
      <div className="flex-1 flex flex-col min-w-0">
        <Header />

        {/* MKT-180 — market-price fetch progress, visible on every page */}
        <PriceFetchProgressBar />

        {/* R3, R4 — update banner between header and content */}
        <UpdateBanner data={updateBannerData} />

        <Content>{children}</Content>
      </div>

      <Snackbar />
    </div>
  );
}

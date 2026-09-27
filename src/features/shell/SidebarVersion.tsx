import { useTranslation } from "react-i18next";

interface SidebarVersionProps {
  appVersion: string;
  /** The build's distribution channel; null for the public build (UPD-031). */
  distributionChannel: string | null;
  isOpen: boolean;
}

/** The version in the sidebar footer, followed by the build's channel when it has one (UPD-031). */
export function SidebarVersion({ appVersion, distributionChannel, isOpen }: SidebarVersionProps) {
  const { t } = useTranslation("common");
  const channelName = distributionChannel
    ? t(`shell.channel_${distributionChannel}`, { defaultValue: distributionChannel })
    : null;
  const tone = isOpen ? "opacity-60" : "opacity-40";

  let version: string;
  if (!isOpen) {
    version = `v${appVersion}`;
  } else if (channelName) {
    version = t("shell.sidebar_version_channel", { version: appVersion, channel: channelName });
  } else {
    version = t("shell.sidebar_version", { version: appVersion });
  }

  return (
    <div className="px-3 py-4 flex flex-col items-center justify-center min-h-12">
      <span
        className={`font-mono text-[14px] tracking-tight transition-opacity duration-300 ${tone}`}
      >
        {version}
      </span>
      {!isOpen && channelName && (
        <span className={`font-mono text-[11px] tracking-tight ${tone}`}>{channelName}</span>
      )}
    </div>
  );
}

import { User } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import {
  overviewValueText,
  overviewYtdText,
  overviewYtdTone,
  profileDisplayName,
} from "../shared/presenter";

const TONE_CLASS = {
  neutral: "text-m3-on-surface-variant",
  loss: "text-m3-loss",
  gain: "text-m3-gain",
} as const;

import { useProfilesOverview } from "./useProfilesOverview";

/**
 * PFL-055 (#768, the validated mock) — every profile with its accounts, value and performance
 * this year, each within its profile; no total: different people's money is never added.
 * A click on a profile makes it the one in use and opens its accounts.
 */
export function ProfilesOverview() {
  const { t } = useTranslation();
  const { overview, error, open } = useProfilesOverview();

  useEffect(() => {
    logger.info("[ProfilesOverview] mounted");
  }, []);

  const defaultName = t("profile.default_name");

  return (
    <div className="p-6 flex flex-col gap-4">
      {error && (
        <p id="profiles-overview-error" role="alert" className="text-sm text-m3-error">
          {t(error.key, error.vars)}
        </p>
      )}
      {overview === null && error === null ? (
        <p id="profiles-overview-loading" className="text-sm text-m3-on-surface-variant">
          {t("profile.overview.loading")}
        </p>
      ) : overview === null ? null : (
        <>
          <p className="text-sm text-m3-on-surface-variant">
            {t("profile.overview.intro", { currency: overview.currency })}
          </p>
          <table id="profiles-overview-table" className="w-full">
            <thead>
              <tr>
                <th scope="col" className="m3-th text-left">
                  {t("profile.overview.profile")}
                </th>
                <th scope="col" className="m3-th text-right">
                  {t("profile.overview.accounts")}
                </th>
                <th scope="col" className="m3-th text-right">
                  {t("profile.overview.value")}
                </th>
                <th scope="col" className="m3-th text-right">
                  {t("profile.overview.ytd")}
                </th>
              </tr>
            </thead>
            <tbody>
              {overview.rows.map((row) => {
                const active = row.profile.id === overview.in_use;
                return (
                  <tr
                    key={row.profile.id}
                    id={`profiles-overview-row-${row.profile.id}`}
                    className="m3-tr hover:bg-m3-on-surface/5"
                  >
                    <td className="m3-td">
                      <button
                        type="button"
                        id={`profiles-overview-open-${row.profile.id}`}
                        aria-current={active ? "true" : undefined}
                        onClick={() => void open(row.profile.id)}
                        className="flex items-center gap-2 font-medium text-m3-on-surface hover:text-m3-primary"
                      >
                        <User size={16} />
                        {profileDisplayName(row.profile, defaultName)}
                        {active && (
                          <span className="rounded-md bg-m3-primary-container px-1.5 text-xs text-m3-on-primary-container">
                            {t("profile.overview.active")}
                          </span>
                        )}
                      </button>
                    </td>
                    <td className="m3-td text-right tabular-nums">{row.account_count}</td>
                    <td className="m3-td text-right tabular-nums">
                      {overviewValueText(row.value)}
                      {row.value_incomplete && (
                        <span
                          className="ml-1 text-xs text-m3-on-surface-variant"
                          title={t("profile.overview.partial_hint")}
                        >
                          {t("profile.overview.partial")}
                        </span>
                      )}
                    </td>
                    <td
                      className={`m3-td text-right tabular-nums ${
                        TONE_CLASS[overviewYtdTone(row.ytd_performance_pct)]
                      }`}
                    >
                      {overviewYtdText(row.ytd_performance_pct)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

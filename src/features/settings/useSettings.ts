import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLanguageOverride, resolveBrowserLang, setLanguageOverride } from "@/i18n/config";
import { getAutoFetch, setAutoFetch } from "@/lib/autoFetchStorage";
import { getAutoRecordPrice, setAutoRecordPrice } from "@/lib/autoRecordPriceStorage";

export type LanguageChoice = "auto" | "en" | "fr";

export function useSettings() {
  const { i18n } = useTranslation();

  const [currentChoice, setCurrentChoice] = useState<LanguageChoice>(
    () => (getLanguageOverride() as LanguageChoice | null) ?? "auto",
  );

  const [autoRecordPrice, setAutoRecordPriceState] = useState<boolean>(() => getAutoRecordPrice());
  const [autoFetch, setAutoFetchState] = useState<boolean>(() => getAutoFetch());

  const setLanguage = useCallback(
    (choice: LanguageChoice) => {
      setCurrentChoice(choice);
      if (choice === "auto") {
        setLanguageOverride(null);
        void i18n.changeLanguage(resolveBrowserLang());
      } else {
        setLanguageOverride(choice);
        void i18n.changeLanguage(choice);
      }
    },
    [i18n],
  );

  const toggleAutoRecordPrice = useCallback(() => {
    setAutoRecordPriceState((current) => {
      const next = !current;
      setAutoRecordPrice(next);
      return next;
    });
  }, []);

  const toggleAutoFetch = useCallback(() => {
    setAutoFetchState((current) => {
      const next = !current;
      setAutoFetch(next);
      return next;
    });
  }, []);

  return {
    currentChoice,
    setLanguage,
    autoRecordPrice,
    toggleAutoRecordPrice,
    autoFetch,
    toggleAutoFetch,
  };
}

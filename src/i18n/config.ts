import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { setDisplayLocale } from "@/lib/microUnits";
import commonEn from "./locales/en/common.json";
import commonFr from "./locales/fr/common.json";

const SUPPORTED_LANGS = ["fr", "en"];
const LANG_OVERRIDE_KEY = "language_override";

export function resolveBrowserLang(): string {
  const [browserLang = "en"] = navigator.language.split("-");
  return SUPPORTED_LANGS.includes(browserLang) ? browserLang : "en";
}

function resolveInitialLang(): string {
  const stored = localStorage.getItem(LANG_OVERRIDE_KEY);
  if (stored && SUPPORTED_LANGS.includes(stored)) return stored;
  return resolveBrowserLang();
}

export function setLanguageOverride(lang: string | null): void {
  if (lang === null) {
    localStorage.removeItem(LANG_OVERRIDE_KEY);
  } else {
    localStorage.setItem(LANG_OVERRIDE_KEY, lang);
  }
}

export function getLanguageOverride(): string | null {
  const stored = localStorage.getItem(LANG_OVERRIDE_KEY);
  return stored && SUPPORTED_LANGS.includes(stored) ? stored : null;
}

i18n.use(initReactI18next).init({
  lng: resolveInitialLang(),
  fallbackLng: "en",
  defaultNS: "common",
  ns: ["common"],
  resources: {
    fr: { common: commonFr },
    en: { common: commonEn },
  },
  interpolation: {
    escapeValue: false,
  },
});

setDisplayLocale(i18n.language);
i18n.on("languageChanged", setDisplayLocale);

export default i18n;

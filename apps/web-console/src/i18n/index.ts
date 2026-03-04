import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enUSCommon from "./locales/en-US/common.json";
import zhCNCommon from "./locales/zh-CN/common.json";

const I18N_LANGUAGE_STORAGE_KEY = "cloudops.console.profile.language";
const SUPPORTED_LANGUAGES = ["en-US", "zh-CN"] as const;

type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

function isSupportedLanguage(value: string): value is SupportedLanguage {
  return SUPPORTED_LANGUAGES.some((item) => item === value);
}

function normalizeLanguage(value: string | null | undefined): SupportedLanguage {
  if (!value) {
    return "en-US";
  }
  const normalized = value.trim();
  if (isSupportedLanguage(normalized)) {
    return normalized;
  }
  if (normalized.toLowerCase().startsWith("zh")) {
    return "zh-CN";
  }
  return "en-US";
}

function readStoredLanguagePreference(): SupportedLanguage | null {
  if (typeof window === "undefined") {
    return null;
  }
  const raw = window.localStorage.getItem(I18N_LANGUAGE_STORAGE_KEY);
  if (!raw) {
    return null;
  }
  return normalizeLanguage(raw);
}

function detectInitialLanguage(): SupportedLanguage {
  const stored = readStoredLanguagePreference();
  if (stored) {
    return stored;
  }

  if (typeof navigator !== "undefined" && typeof navigator.language === "string") {
    return normalizeLanguage(navigator.language);
  }

  return "en-US";
}

function persistLanguagePreference(language: string): void {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(I18N_LANGUAGE_STORAGE_KEY, normalizeLanguage(language));
}

void i18n.use(initReactI18next).init({
  lng: detectInitialLanguage(),
  fallbackLng: "en-US",
  supportedLngs: [...SUPPORTED_LANGUAGES],
  load: "currentOnly",
  interpolation: {
    escapeValue: false
  },
  resources: {
    "en-US": {
      common: enUSCommon
    },
    "zh-CN": {
      common: zhCNCommon
    }
  },
  defaultNS: "common"
});

i18n.on("languageChanged", persistLanguagePreference);

export default i18n;

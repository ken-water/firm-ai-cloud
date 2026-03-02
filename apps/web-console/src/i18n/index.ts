import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enUSCommon from "./locales/en-US/common.json";

void i18n.use(initReactI18next).init({
  lng: "en-US",
  fallbackLng: "en-US",
  interpolation: {
    escapeValue: false
  },
  resources: {
    "en-US": {
      common: enUSCommon
    }
  },
  defaultNS: "common"
});

export default i18n;

import { useTranslation } from "react-i18next";

export function App() {
  const { t } = useTranslation();

  return (
    <main style={{ fontFamily: "sans-serif", padding: "2rem", lineHeight: 1.5 }}>
      <h1>{t("app.title")}</h1>
      <p>{t("app.subtitle")}</p>
      <ul>
        <li>{t("app.points.cmdb")}</li>
        <li>{t("app.points.monitoring")}</li>
        <li>{t("app.points.ticketing")}</li>
      </ul>
    </main>
  );
}

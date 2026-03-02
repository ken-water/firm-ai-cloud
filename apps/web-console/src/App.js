import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useTranslation } from "react-i18next";
export function App() {
    const { t } = useTranslation();
    return (_jsxs("main", { style: { fontFamily: "sans-serif", padding: "2rem", lineHeight: 1.5 }, children: [_jsx("h1", { children: t("app.title") }), _jsx("p", { children: t("app.subtitle") }), _jsxs("ul", { children: [_jsx("li", { children: t("app.points.cmdb") }), _jsx("li", { children: t("app.points.monitoring") }), _jsx("li", { children: t("app.points.ticketing") })] })] }));
}

import type { ReactNode } from "react";

export type NavigationItem = {
  href: string;
  label: string;
  active?: boolean;
  group?: string;
};
export type SectionTabItem = {
  key: string;
  label: string;
  active?: boolean;
};

type AuthGateProps = {
  title: string;
  subtitle: string;
  notice?: string | null;
  error?: string | null;
  children: ReactNode;
};

type AppShellProps = {
  title: string;
  subtitle: string;
  statusText: string;
  modeText: string;
  signOutLabel: string;
  onSignOut: () => void;
  navigationItems: NavigationItem[];
  secondaryNavigationItems?: NavigationItem[];
  sectionTabs?: SectionTabItem[];
  onSelectSectionTab?: (key: string) => void;
  notice?: string | null;
  error?: string | null;
  warning?: string | null;
  topbarActions?: ReactNode;
  children: ReactNode;
};

type SectionCardProps = {
  id?: string;
  title?: string;
  actions?: ReactNode;
  children: ReactNode;
};

export function AuthGate(props: AuthGateProps) {
  const { title, subtitle, notice, error, children } = props;
  return (
    <main className="auth-gate">
      <header className="auth-gate-header">
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </header>

      {notice && <p className="banner banner-success">{notice}</p>}
      {error && <p className="banner banner-error">{error}</p>}

      <section className="auth-card">{children}</section>
    </main>
  );
}

export function AppShell(props: AppShellProps) {
  const {
    title,
    subtitle,
    statusText,
    modeText,
    signOutLabel,
    onSignOut,
    navigationItems,
    secondaryNavigationItems,
    sectionTabs,
    onSelectSectionTab,
    notice,
    error,
    warning,
    topbarActions,
    children
  } = props;

  return (
    <main className="app-shell">
      <aside className="app-sidebar">
        <div className="sidebar-brand">
          <h1>{title}</h1>
          <p>{subtitle}</p>
        </div>
        <nav className="sidebar-nav">
          {(() => {
            const grouped = new Map<string, NavigationItem[]>();
            navigationItems.forEach((item) => {
              const key = item.group ?? "General";
              const existing = grouped.get(key) ?? [];
              existing.push(item);
              grouped.set(key, existing);
            });
            return [...grouped.entries()].map(([groupName, items]) => (
              <div key={`sidebar-group-${groupName}`} className="sidebar-nav-group">
                <p className="sidebar-nav-group-title">{groupName}</p>
                {items.map((item) => (
                  <a
                    key={item.href}
                    href={item.href}
                    className={item.active ? "is-active" : undefined}
                    aria-current={item.active ? "page" : undefined}
                  >
                    {item.label}
                  </a>
                ))}
              </div>
            ));
          })()}
        </nav>
      </aside>

      <div className="app-main">
        <header className="app-topbar">
          <div>
            <strong>{statusText}</strong>
            <p>{modeText}</p>
          </div>
          <div className="topbar-actions">
            {topbarActions ?? <button onClick={onSignOut}>{signOutLabel}</button>}
          </div>
        </header>
        {secondaryNavigationItems && secondaryNavigationItems.length > 0 && (
          <div className="app-subnav-tabs">
            {secondaryNavigationItems.map((item) => (
              <a
                key={`subnav-${item.href}`}
                href={item.href}
                className={item.active ? "is-active" : undefined}
                aria-current={item.active ? "page" : undefined}
              >
                {item.label}
              </a>
            ))}
          </div>
        )}
        {sectionTabs && sectionTabs.length > 0 && (
          <div className="app-section-tabs">
            {sectionTabs.map((tab) => (
              <button
                key={`section-tab-${tab.key}`}
                className={tab.active ? "is-active" : undefined}
                onClick={() => onSelectSectionTab?.(tab.key)}
              >
                {tab.label}
              </button>
            ))}
          </div>
        )}

        {notice && <p className="banner banner-success">{notice}</p>}
        {error && <p className="banner banner-error">{error}</p>}
        {warning && <p className="banner banner-warn">{warning}</p>}

        {children}
      </div>
    </main>
  );
}

export function SectionCard(props: SectionCardProps) {
  const { id, title, actions, children } = props;
  return (
    <section id={id} className="section-card">
      {(title || actions) && (
        <div className="section-head">
          {title && <h2 className="section-title">{title}</h2>}
          {actions && <div className="section-actions">{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}

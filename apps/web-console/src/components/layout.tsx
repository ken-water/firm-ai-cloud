import type { ReactNode } from "react";

export type NavigationItem = {
  href: string;
  label: string;
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
  notice?: string | null;
  error?: string | null;
  warning?: string | null;
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
    notice,
    error,
    warning,
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
          {navigationItems.map((item) => (
            <a key={item.href} href={item.href}>
              {item.label}
            </a>
          ))}
        </nav>
      </aside>

      <div className="app-main">
        <header className="app-topbar">
          <div>
            <strong>{statusText}</strong>
            <p>{modeText}</p>
          </div>
          <button onClick={onSignOut}>{signOutLabel}</button>
        </header>

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

const DEFAULT_API_BASE_URL =
  typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.hostname}:8080`
    : "http://127.0.0.1:8080";
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL?.trim() || DEFAULT_API_BASE_URL;
const API_AUTH_USER = (import.meta.env.VITE_AUTH_USER ?? "admin").trim();
const API_AUTH_TOKEN = (import.meta.env.VITE_AUTH_TOKEN ?? "").trim();
const AUTH_SESSION_STORAGE_KEY = "cloudops.auth.session.v1";
export const AUTH_SESSION_EXPIRED_EVENT = "cloudops.auth.session-expired";

export const DEFAULT_AUTH_USER = API_AUTH_USER;
export const DEFAULT_AUTH_TOKEN = API_AUTH_TOKEN;

export type AuthMode = "header" | "bearer";

export type AuthSession = {
  mode: AuthMode;
  principal: string;
  token: string | null;
};

const runtimeDefaultSession = deriveDefaultAuthSession();
let runtimeAuthSession: AuthSession | null = loadStoredAuthSession() ?? runtimeDefaultSession;

export function getRuntimeAuthSession(): AuthSession | null {
  return runtimeAuthSession;
}

export function setRuntimeAuthSession(session: AuthSession | null): void {
  runtimeAuthSession = session;
  persistAuthSession(session);
}

export async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const headers = new Headers(init?.headers ?? undefined);
  if (runtimeAuthSession?.mode === "header") {
    const principal = runtimeAuthSession.principal.trim();
    if (principal.length > 0) {
      headers.set("x-auth-user", principal);
    }
  }
  if (runtimeAuthSession?.mode === "bearer") {
    const token = runtimeAuthSession.token?.trim() ?? "";
    if (token.length > 0) {
      headers.set("Authorization", `Bearer ${token}`);
    }
  }

  const response = await fetch(input, {
    ...init,
    headers
  });

  if (response.status === 403 && runtimeAuthSession?.mode === "bearer") {
    const message = await extractApiErrorMessage(response.clone());
    if (message && isSessionExpiredError(message)) {
      setRuntimeAuthSession(null);
      if (typeof window !== "undefined") {
        window.dispatchEvent(new Event(AUTH_SESSION_EXPIRED_EVENT));
      }
    }
  }

  return response;
}

export async function readErrorMessage(response: Response): Promise<string> {
  const message = await extractApiErrorMessage(response.clone());
  if (response.status === 403) {
    if (message && isSessionExpiredError(message)) {
      return "Session is invalid or expired. Please sign in again.";
    }
    if (message) {
      return `Unauthorized: ${message}`;
    }
    return "Unauthorized: your current role cannot perform this action.";
  }

  return message ?? `HTTP ${response.status}`;
}

function deriveDefaultAuthSession(): AuthSession | null {
  if (API_AUTH_TOKEN.length > 0) {
    return {
      mode: "bearer",
      principal: "oidc-session",
      token: API_AUTH_TOKEN
    };
  }

  if (API_AUTH_USER.length > 0) {
    return {
      mode: "header",
      principal: API_AUTH_USER,
      token: null
    };
  }

  return null;
}

function loadStoredAuthSession(): AuthSession | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(AUTH_SESSION_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") {
      return null;
    }

    const mode = (parsed as { mode?: unknown }).mode;
    const principal = (parsed as { principal?: unknown }).principal;
    const token = (parsed as { token?: unknown }).token;

    if (mode === "header" && typeof principal === "string" && principal.trim().length > 0) {
      return {
        mode: "header",
        principal: principal.trim(),
        token: null
      };
    }

    if (mode === "bearer" && typeof token === "string" && token.trim().length > 0) {
      return {
        mode: "bearer",
        principal: typeof principal === "string" ? principal : "oidc-session",
        token: token.trim()
      };
    }
  } catch {
    return null;
  }

  return null;
}

function persistAuthSession(session: AuthSession | null): void {
  if (typeof window === "undefined") {
    return;
  }

  if (!session) {
    window.localStorage.removeItem(AUTH_SESSION_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(AUTH_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function isSessionExpiredError(message: string): boolean {
  const normalized = message.toLowerCase();
  return (
    normalized.includes("invalid or expired")
    || normalized.includes("bearer token cannot be empty")
    || normalized.includes("authorization header is invalid")
  );
}

async function extractApiErrorMessage(response: Response): Promise<string | null> {
  try {
    const payload = (await response.json()) as unknown;
    if (payload && typeof payload === "object" && "error" in payload) {
      const value = (payload as { error?: unknown }).error;
      if (typeof value === "string" && value.trim().length > 0) {
        return value.trim();
      }
    }
  } catch {
    return null;
  }

  return null;
}

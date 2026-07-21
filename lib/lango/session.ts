/// Real per-user session storage (chat feature, Phase 4). This dashboard's
/// existing views have no login of their own — they authenticate
/// transparently as a fixed demo account (see api-client.ts's
/// DEMO_EMAIL/DEMO_PASSWORD) so the deployed mock-data demo and local dev
/// both work with zero setup. That convenience is left completely intact.
///
/// This module exists ONLY for the new /login -> /chat path: a real user
/// role (staff vs. department_reviewer vs. compliance_admin) has to come
/// from somewhere for role-gated landing to mean anything, and the demo
/// account is always compliance_admin. No new auth mechanism is introduced
/// — this stores exactly what the EXISTING `POST /api/auth/login` endpoint
/// already returns (see backend/src/models.rs's `LoginResponse`), in
/// `localStorage` so it survives a page reload/direct navigation to /chat.
export interface LangoUser {
  id: string;
  email: string;
  department: string;
  role: "staff" | "department_reviewer" | "compliance_admin";
  organisation_id: string;
}

interface StoredSession {
  token: string;
  user: LangoUser;
}

const SESSION_KEY = "lango_session";

export function saveSession(token: string, user: LangoUser): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(SESSION_KEY, JSON.stringify({ token, user }));
}

export function getSession(): StoredSession | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(SESSION_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as StoredSession;
  } catch {
    return null;
  }
}

export function clearSession(): void {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(SESSION_KEY);
}

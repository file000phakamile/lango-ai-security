// background.js — MV3 service worker.
//
// Content scripts run inside chatgpt.com's page context and can't make
// cross-origin fetch() calls to the Lango API directly under Manifest V3
// (they're subject to the page's own CSP/origin restrictions). This service
// worker holds the actual fetch() calls and the stored JWT; content scripts
// talk to it via chrome.runtime.sendMessage. Extension-context fetches with
// the right host_permissions bypass normal browser CORS enforcement, so this
// works regardless of the backend's CORS_ORIGIN setting (that setting exists
// for the Next.js frontend's browser-context fetches, not this).

const DEFAULT_API_BASE_URL = "https://lango-backend-qwkx.onrender.com";

async function getSettings() {
  const stored = await chrome.storage.local.get([
    "jwt",
    "apiBaseUrl",
    "scanCount",
    "user",
    "requiresConsent",
    "consentPolicyVersion",
  ]);
  return {
    jwt: stored.jwt ?? null,
    apiBaseUrl: stored.apiBaseUrl || DEFAULT_API_BASE_URL,
    scanCount: stored.scanCount ?? 0,
    user: stored.user ?? null,
    // Part 4 of the multi-tenancy change: a brand-new user in a brand-new
    // organisation must accept a data-use consent screen before they can
    // scan anything. Stored here (not just returned once from login) so
    // reopening the popup without a fresh login still shows the consent
    // screen if it was never actually acknowledged.
    requiresConsent: stored.requiresConsent ?? false,
    consentPolicyVersion: stored.consentPolicyVersion ?? null,
  };
}

async function scanPrompt(prompt) {
  const { jwt, apiBaseUrl, scanCount } = await getSettings();
  if (!jwt) {
    return { ok: false, error: "not_authenticated", message: "Not logged in — open the Lango extension options." };
  }

  let res;
  try {
    res = await fetch(`${apiBaseUrl}/api/scan`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${jwt}`,
      },
      body: JSON.stringify({ prompt }),
    });
  } catch (err) {
    // Network error, DNS failure, or a Render free-tier cold-start timeout —
    // fail CLOSED. The caller (content script) must not send the prompt.
    // This matches the fail-closed principle documented in
    // docs/ARCHITECTURE.md and docs/SECURITY_PRIVACY.md: an unscanned prompt
    // must never go through just because the scanner itself was unreachable.
    return { ok: false, error: "network_error", message: String(err?.message ?? err) };
  }

  if (res.status === 401) {
    // Stored token is invalid/expired (backend JWTs are valid 12h — see
    // backend/src/auth.rs SESSION_TTL_HOURS). Clear it so the popup/options
    // page correctly show "not logged in" instead of a stale "connected"
    // status, and so the next scan attempt fails closed with a clear reason
    // instead of repeatedly hitting a 401.
    await chrome.storage.local.remove("jwt");
    return { ok: false, error: "not_authenticated", message: "Login expired — log in again via extension options." };
  }

  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    let code = null;
    try {
      const body = await res.json();
      message = body?.error?.message ?? message;
      code = body?.error?.code ?? null;
    } catch {
      // non-JSON error body — keep the HTTP-status message
    }
    // Distinct from a generic api_error so the popup/content script can
    // react specifically (open the consent screen) rather than showing a
    // generic failure banner — see routes/consent.rs and routes/scan.rs's
    // consent gate on the backend side.
    if (code === "CONSENT_REQUIRED") {
      return { ok: false, error: "consent_required", message };
    }
    return { ok: false, error: "api_error", message };
  }

  const data = await res.json();

  // Counted here, not in the content script, so a page reload can't lose
  // the count and a retry that never reached the API can't inflate it.
  await chrome.storage.local.set({ scanCount: scanCount + 1 });

  return { ok: true, data };
}

async function login(email, password, apiBaseUrl) {
  let res;
  try {
    res = await fetch(`${apiBaseUrl}/api/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });
  } catch (err) {
    return { ok: false, message: `Could not reach ${apiBaseUrl}: ${err?.message ?? err}` };
  }

  if (!res.ok) {
    let message = `Login failed (HTTP ${res.status})`;
    try {
      const body = await res.json();
      message = body?.error?.message ?? message;
    } catch {
      // ignore non-JSON error body
    }
    return { ok: false, message };
  }

  const body = await res.json();
  await chrome.storage.local.set({
    jwt: body.token,
    apiBaseUrl,
    user: body.user,
    requiresConsent: Boolean(body.requires_consent),
    consentPolicyVersion: body.consent_policy_version ?? null,
  });
  return {
    ok: true,
    user: body.user,
    requiresConsent: Boolean(body.requires_consent),
    consentPolicyVersion: body.consent_policy_version ?? null,
  };
}

async function acceptConsent() {
  const { jwt, apiBaseUrl, consentPolicyVersion } = await getSettings();
  if (!jwt) {
    return { ok: false, message: "Not logged in." };
  }
  if (!consentPolicyVersion) {
    return { ok: false, message: "No pending consent policy version to accept." };
  }

  let res;
  try {
    res = await fetch(`${apiBaseUrl}/api/consent/accept`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${jwt}`,
      },
      body: JSON.stringify({ policy_version: consentPolicyVersion }),
    });
  } catch (err) {
    return { ok: false, message: `Could not reach ${apiBaseUrl}: ${err?.message ?? err}` };
  }

  if (!res.ok) {
    let message = `Consent could not be recorded (HTTP ${res.status})`;
    try {
      const body = await res.json();
      message = body?.error?.message ?? message;
    } catch {
      // ignore non-JSON error body
    }
    return { ok: false, message };
  }

  // Consent accepted — clear the pending flag so the popup switches back
  // to its normal connected view immediately, without needing a fresh login.
  await chrome.storage.local.set({ requiresConsent: false });
  return { ok: true };
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === "LANGO_SCAN_PROMPT") {
    scanPrompt(message.prompt).then(sendResponse);
    return true; // keep the message channel open for the async response
  }
  if (message?.type === "LANGO_LOGIN") {
    login(message.email, message.password, message.apiBaseUrl).then(sendResponse);
    return true;
  }
  if (message?.type === "LANGO_LOGOUT") {
    chrome.storage.local.remove(["jwt", "user", "requiresConsent", "consentPolicyVersion"]).then(() =>
      sendResponse({ ok: true }),
    );
    return true;
  }
  if (message?.type === "LANGO_ACCEPT_CONSENT") {
    acceptConsent().then(sendResponse);
    return true;
  }
  if (message?.type === "LANGO_GET_STATUS") {
    getSettings().then((s) =>
      sendResponse({
        loggedIn: Boolean(s.jwt),
        apiBaseUrl: s.apiBaseUrl,
        scanCount: s.scanCount,
        user: s.user,
        requiresConsent: s.requiresConsent,
        consentPolicyVersion: s.consentPolicyVersion,
      }),
    );
    return true;
  }
  return false;
});

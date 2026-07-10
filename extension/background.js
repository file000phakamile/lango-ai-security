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
  const stored = await chrome.storage.local.get(["jwt", "apiBaseUrl", "scanCount", "user"]);
  return {
    jwt: stored.jwt ?? null,
    apiBaseUrl: stored.apiBaseUrl || DEFAULT_API_BASE_URL,
    scanCount: stored.scanCount ?? 0,
    user: stored.user ?? null,
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
    try {
      const body = await res.json();
      message = body?.error?.message ?? message;
    } catch {
      // non-JSON error body — keep the HTTP-status message
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
  await chrome.storage.local.set({ jwt: body.token, apiBaseUrl, user: body.user });
  return { ok: true, user: body.user };
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
    chrome.storage.local.remove(["jwt", "user"]).then(() => sendResponse({ ok: true }));
    return true;
  }
  if (message?.type === "LANGO_GET_STATUS") {
    getSettings().then((s) =>
      sendResponse({
        loggedIn: Boolean(s.jwt),
        apiBaseUrl: s.apiBaseUrl,
        scanCount: s.scanCount,
        user: s.user,
      }),
    );
    return true;
  }
  return false;
});

const DEFAULT_API_BASE_URL = "https://lango-backend-qwkx.onrender.com";

// Kept in sync with manifest.json's content_scripts matches by hand — only
// five entries, so a build step to derive this list felt like more
// machinery than the situation warrants. `verified` must stay honest:
// chatgpt.com's prompt-side interception and gemini.google.com (both prompt
// and response scanning) have actually been driven against a live session —
// see Questions.md items 26/31/34. claude.ai was re-checked live in the
// UI-copy pass (a real fetch and a real headless-browser navigation both
// still return a 403/Cloudflare block, unchanged from every earlier
// attempt) and stays unverified — see Questions.md item 37 for why this
// stays false even though it was instructed to be marked verified.
const SUPPORTED_SITES = [
  { host: "chatgpt.com", label: "ChatGPT", verified: true },
  { host: "claude.ai", label: "Claude", verified: false },
  { host: "gemini.google.com", label: "Gemini", verified: true },
  { host: "chat.deepseek.com", label: "DeepSeek", verified: false },
  { host: "copilot.microsoft.com", label: "Copilot", verified: false },
];

// UI copy pass, Part 3: a staff-role user has no dashboard access in the
// real, intended access model (see docs/ARCHITECTURE.md's role
// definitions), even though today's actual deployed frontend doesn't
// enforce that yet (see Questions.md item 38) — so this link must not
// assume dashboard access every logged-in user actually has. Defaults to
// the public, unauthenticated GitHub-hosted copy (safe before login too,
// when the role isn't known yet); only switches to the dashboard's own
// Help tab for a role that genuinely has dashboard access.
const PUBLIC_HELP_URL = "https://github.com/file000phakamile/lango-ai-security/blob/main/HOW_TO_USE.md";
const DASHBOARD_HELP_URL = "https://lango-app-dusky.vercel.app/#help";
const DASHBOARD_ROLES = ["compliance_admin", "department_reviewer"];

const tabStatusEl = document.getElementById("tabStatus");
const statusDot = document.getElementById("statusDot");
const statusText = document.getElementById("statusText");
const loggedOutView = document.getElementById("loggedOutView");
const loggedInView = document.getElementById("loggedInView");
const consentView = document.getElementById("consentView");
const consentVersionText = document.getElementById("consentVersionText");
const consentMessage = document.getElementById("consentMessage");
const scanCountEl = document.getElementById("scanCount");
const emailInput = document.getElementById("email");
const passwordInput = document.getElementById("password");
const loginMessage = document.getElementById("loginMessage");
const helpLink = document.getElementById("helpLink");

// Reports on the CURRENT tab specifically — distinct from the static
// "Active on: ..." site list above, which just states what this extension
// supports in general. This actually queries whether the Lango content
// script is running on the page open right now, rather than assuming it
// from the URL alone (a content script can fail to inject or fail to
// initialize even on a matching URL, e.g. if the page loaded before the
// extension did).
function refreshTabStatus() {
  chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    const tab = tabs && tabs[0];
    let hostname = null;
    try {
      hostname = tab && tab.url ? new URL(tab.url).hostname : null;
    } catch {
      hostname = null;
    }
    const match = hostname && SUPPORTED_SITES.find((s) => hostname === s.host || hostname.endsWith(`.${s.host}`));

    if (!tab || !tab.id || !match) {
      tabStatusEl.textContent = "This tab: not a Lango-supported site";
      return;
    }

    chrome.tabs.sendMessage(tab.id, { type: "LANGO_PING" }, (resp) => {
      // chrome.runtime.lastError is set (not thrown) when there's no
      // listener on the other end — e.g. the content script hasn't loaded
      // yet, or failed during its own initialization. Reading it here is
      // required to prevent Chrome from logging an "Unchecked runtime.lastError"
      // warning to the console even though we're handling the failure case
      // explicitly below.
      const injected = !chrome.runtime.lastError && resp && resp.siteName;
      if (injected) {
        tabStatusEl.textContent = match.verified
          ? `Active on this tab: ${match.label} (verified)`
          : `Active on this tab: ${match.label} (unverified adapter — test before relying on it)`;
      } else {
        tabStatusEl.textContent = `${match.label} detected, but Lango isn't responding on this tab yet — try reloading the page.`;
      }
    });
  });
}

function setLoginMessage(text, kind) {
  loginMessage.textContent = text || "";
  loginMessage.className = kind || "";
}

function render(status) {
  if (!status) {
    statusText.textContent = "Extension error";
    return;
  }

  scanCountEl.textContent = status.scanCount;

  // UI copy pass, Part 3: point a staff-role user at the public help copy,
  // not the dashboard — see the constants' own comment above for why.
  const role = status.user && status.user.role;
  helpLink.href = role && DASHBOARD_ROLES.includes(role) ? DASHBOARD_HELP_URL : PUBLIC_HELP_URL;

  if (status.loggedIn && status.requiresConsent) {
    // Logged in, but this user has never accepted (or needs to re-accept)
    // their organisation's data-use consent policy — takes priority over
    // the normal loggedInView. Nothing is sent to the AI Gateway for this
    // user until they acknowledge this screen (enforced server-side too —
    // see routes/scan.rs's consent gate, not just this UI).
    statusDot.classList.remove("off");
    statusDot.classList.add("on");
    statusText.textContent = status.user ? `Connected as ${status.user.email}` : "Connected";
    consentVersionText.textContent = status.consentPolicyVersion || "—";
    consentView.classList.remove("hidden");
    loggedInView.classList.add("hidden");
    loggedOutView.classList.add("hidden");
  } else if (status.loggedIn) {
    statusDot.classList.remove("off");
    statusDot.classList.add("on");
    statusText.textContent = status.user ? `Connected as ${status.user.email}` : "Connected";
    loggedInView.classList.remove("hidden");
    consentView.classList.add("hidden");
    loggedOutView.classList.add("hidden");
  } else {
    statusDot.classList.remove("on");
    statusDot.classList.add("off");
    statusText.textContent = "Not logged in";
    loggedInView.classList.add("hidden");
    consentView.classList.add("hidden");
    loggedOutView.classList.remove("hidden");
  }
}

function refreshStatus() {
  chrome.runtime.sendMessage({ type: "LANGO_GET_STATUS" }, render);
}

refreshStatus();
refreshTabStatus();

document.getElementById("loginBtn").addEventListener("click", async () => {
  const email = emailInput.value.trim();
  const password = passwordInput.value;

  if (!email || !password) {
    setLoginMessage("Email and password are both required.", "error");
    return;
  }

  setLoginMessage("Logging in…", "");
  // Uses whatever API base URL is currently stored (the live Render backend
  // by default), so the popup's login form works with zero extra clicks for
  // the common case. Overriding it for local dev is one click away via
  // "Advanced" below, not required for normal use.
  const stored = await chrome.storage.local.get(["apiBaseUrl"]);
  const apiBaseUrl = stored.apiBaseUrl || DEFAULT_API_BASE_URL;

  chrome.runtime.sendMessage({ type: "LANGO_LOGIN", email, password, apiBaseUrl }, (result) => {
    if (result && result.ok) {
      passwordInput.value = "";
      setLoginMessage("", "");
      refreshStatus();
    } else {
      setLoginMessage((result && result.message) || "Login failed.", "error");
    }
  });
});

document.getElementById("acceptConsentBtn").addEventListener("click", () => {
  consentMessage.textContent = "Recording…";
  consentMessage.className = "";
  chrome.runtime.sendMessage({ type: "LANGO_ACCEPT_CONSENT" }, (result) => {
    if (result && result.ok) {
      consentMessage.textContent = "";
      refreshStatus();
    } else {
      consentMessage.textContent = (result && result.message) || "Could not record consent.";
      consentMessage.className = "error";
    }
  });
});

document.getElementById("logoutBtn").addEventListener("click", () => {
  chrome.runtime.sendMessage({ type: "LANGO_LOGOUT" }, () => {
    refreshStatus();
  });
});

document.getElementById("advancedLink").addEventListener("click", () => {
  chrome.runtime.openOptionsPage();
});

// Enter-to-submit in either field, same as any normal login form.
[emailInput, passwordInput].forEach((el) => {
  el.addEventListener("keydown", (e) => {
    if (e.key === "Enter") document.getElementById("loginBtn").click();
  });
});

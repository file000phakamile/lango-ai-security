const DEFAULT_API_BASE_URL = "https://lango-backend-qwkx.onrender.com";

const statusDot = document.getElementById("statusDot");
const statusText = document.getElementById("statusText");
const loggedOutView = document.getElementById("loggedOutView");
const loggedInView = document.getElementById("loggedInView");
const scanCountEl = document.getElementById("scanCount");
const emailInput = document.getElementById("email");
const passwordInput = document.getElementById("password");
const loginMessage = document.getElementById("loginMessage");

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

  if (status.loggedIn) {
    statusDot.classList.remove("off");
    statusDot.classList.add("on");
    statusText.textContent = status.user ? `Connected as ${status.user.email}` : "Connected";
    loggedInView.classList.remove("hidden");
    loggedOutView.classList.add("hidden");
  } else {
    statusDot.classList.remove("on");
    statusDot.classList.add("off");
    statusText.textContent = "Not logged in";
    loggedInView.classList.add("hidden");
    loggedOutView.classList.remove("hidden");
  }
}

function refreshStatus() {
  chrome.runtime.sendMessage({ type: "LANGO_GET_STATUS" }, render);
}

refreshStatus();

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

const DEFAULT_API_BASE_URL = "https://lango-backend-qwkx.onrender.com";

const apiBaseUrlInput = document.getElementById("apiBaseUrl");
const emailInput = document.getElementById("email");
const passwordInput = document.getElementById("password");
const messageEl = document.getElementById("message");

function setMessage(text, kind) {
  messageEl.textContent = text;
  messageEl.className = kind || "";
}

async function loadStoredSettings() {
  const stored = await chrome.storage.local.get(["apiBaseUrl", "user"]);
  apiBaseUrlInput.value = stored.apiBaseUrl || DEFAULT_API_BASE_URL;
  if (stored.user) {
    setMessage(`Logged in as ${stored.user.email} (${stored.user.role}).`, "success");
  }
}
loadStoredSettings();

document.getElementById("loginBtn").addEventListener("click", () => {
  const apiBaseUrl = apiBaseUrlInput.value.trim().replace(/\/$/, "") || DEFAULT_API_BASE_URL;
  const email = emailInput.value.trim();
  const password = passwordInput.value;

  if (!email || !password) {
    setMessage("Email and password are both required.", "error");
    return;
  }

  setMessage("Logging in…", "");
  chrome.runtime.sendMessage({ type: "LANGO_LOGIN", email, password, apiBaseUrl }, (result) => {
    if (result && result.ok) {
      setMessage(`Logged in as ${result.user.email} (${result.user.role}).`, "success");
      passwordInput.value = "";
    } else {
      setMessage((result && result.message) || "Login failed.", "error");
    }
  });
});

document.getElementById("logoutBtn").addEventListener("click", () => {
  chrome.runtime.sendMessage({ type: "LANGO_LOGOUT" }, () => {
    setMessage("Logged out.", "");
  });
});

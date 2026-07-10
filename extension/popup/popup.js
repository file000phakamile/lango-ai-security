document.getElementById("optionsBtn").addEventListener("click", () => {
  chrome.runtime.openOptionsPage();
});

chrome.runtime.sendMessage({ type: "LANGO_GET_STATUS" }, (status) => {
  const dot = document.getElementById("statusDot");
  const text = document.getElementById("statusText");
  const count = document.getElementById("scanCount");

  if (!status) {
    text.textContent = "Extension error";
    return;
  }

  if (status.loggedIn) {
    dot.classList.remove("off");
    dot.classList.add("on");
    const host = status.apiBaseUrl.replace(/^https?:\/\//, "");
    text.textContent = status.user ? `${status.user.email} — ${host}` : `Connected — ${host}`;
  } else {
    dot.classList.remove("on");
    dot.classList.add("off");
    text.textContent = "Not logged in";
  }

  count.textContent = status.scanCount;
});

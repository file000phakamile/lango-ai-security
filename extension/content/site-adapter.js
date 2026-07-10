// content/site-adapter.js — shared interception orchestration, independent
// of any specific chat site's DOM structure.
//
// A site adapter implements this interface and calls LangoSiteAdapter.init():
//
//   siteName: string
//   findComposer(): HTMLElement | null
//     Returns the current prompt input element (a <textarea> or a
//     contenteditable element), or null if not present on the page right now.
//   findSendButton(composer): HTMLElement | null
//     Returns the button that submits the composer's contents, or null.
//   readText(composer): string
//     Returns the composer's current text content.
//   writeText(composer, text): void
//     Replaces the composer's content with `text`, updating whatever
//     framework state (React, etc.) the site's own JS relies on — see each
//     adapter's own comments for the specific trick this requires.
//
// This split exists so a second site (claude.ai, gemini.google.com) could be
// added later by writing one new adapter file plus one manifest
// content_scripts entry, without touching this file. Only chatgpt.com is
// actually implemented in v0.1 — see extension/README.md and Questions.md
// for why the other sites aren't just stubbed in as well.
//
// Design note on *why* interception happens at the document level via
// capture-phase listeners rather than binding directly to the composer
// element: chatgpt.com is a single-page app that can remount its composer
// (e.g. on navigating to a new conversation). Binding listeners to a cached
// element reference would silently stop working after a remount; delegating
// at the document level and re-querying the DOM on every keydown/click does
// not have that failure mode.

const LangoSiteAdapter = (() => {
  let bypassNextEvent = false;

  function init(adapter) {
    document.addEventListener("keydown", (e) => onKeydown(e, adapter), true);
    document.addEventListener("click", (e) => onClick(e, adapter), true);
    console.info(`[Lango] content script active on ${adapter.siteName}`);
  }

  function onKeydown(e, adapter) {
    if (bypassNextEvent) {
      bypassNextEvent = false;
      return;
    }
    // Enter submits; Shift+Enter is a newline; isComposing guards against
    // IME candidate-confirmation Enter presses being mistaken for submit.
    if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;

    const composer = adapter.findComposer();
    if (!composer || !composer.contains(e.target)) return;

    handleSubmitAttempt(e, adapter, composer);
  }

  function onClick(e, adapter) {
    if (bypassNextEvent) {
      bypassNextEvent = false;
      return;
    }
    const composer = adapter.findComposer();
    if (!composer) return;

    const sendBtn = adapter.findSendButton(composer);
    if (!sendBtn || !sendBtn.contains(e.target)) return;

    handleSubmitAttempt(e, adapter, composer);
  }

  async function handleSubmitAttempt(e, adapter, composer) {
    const text = (adapter.readText(composer) || "").trim();
    if (!text) return; // nothing to scan — let the native (likely no-op) behavior proceed

    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();

    showBanner("Lango: scanning prompt…", "neutral", { autoDismiss: false });

    let response;
    try {
      response = await chrome.runtime.sendMessage({ type: "LANGO_SCAN_PROMPT", prompt: text });
    } catch (err) {
      response = null;
    }

    // Fail CLOSED, not open: chrome.runtime.sendMessage throwing (e.g. the
    // service worker was killed and failed to wake), or the background
    // worker itself reporting ok !== true, both mean the prompt is NOT sent.
    if (!response || response.ok !== true) {
      const detail =
        response?.error === "not_authenticated"
          ? "not logged in — open the Lango extension options"
          : response?.message || "Lango backend unreachable";
      showBanner(`Lango: blocked — ${detail}. Prompt not sent.`, "blocked", { autoDismiss: false });
      return;
    }

    const result = response.data;
    switch (result.decision) {
      case "cleared_no_entities":
        showBanner("Lango: no sensitive entities detected — sending", "cleared", { autoDismiss: true });
        resend(adapter, composer);
        break;

      case "redacted_and_forwarded": {
        adapter.writeText(composer, result.redacted_prompt);
        const n = result.entities_detected ? result.entities_detected.length : 0;
        showBanner(
          `Lango: ${n} entit${n === 1 ? "y" : "ies"} redacted before sending`,
          "redacted",
          { autoDismiss: true },
        );
        // Small delay so the site's own framework (React/etc.) finishes
        // processing the synthetic input event before we resend — resending
        // immediately risks racing against that state update.
        setTimeout(() => resend(adapter, composer), 60);
        break;
      }

      case "blocked_low_confidence":
        // Do NOT resend, do NOT auto-retry. The user must edit the prompt
        // themselves and submit again.
        showBanner(`Lango: blocked — ${result.reason_string}`, "blocked", { autoDismiss: false });
        break;

      default:
        showBanner("Lango: unexpected response from backend — prompt not sent", "blocked", { autoDismiss: false });
    }
  }

  function resend(adapter, composer) {
    bypassNextEvent = true;
    const sendBtn = adapter.findSendButton(composer);
    if (sendBtn && !sendBtn.disabled) {
      sendBtn.click();
    } else {
      // Fallback: re-dispatch Enter on the composer itself. This relies on
      // the site's own JS having a keydown handler that triggers send on
      // Enter (true for chatgpt.com) rather than native browser form-submit
      // behavior (which a <textarea> doesn't have anyway).
      composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    }
  }

  return { init };
})();

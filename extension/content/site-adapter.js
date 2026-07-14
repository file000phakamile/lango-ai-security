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
// Optional, for the three sites response-scanning was added to (product-
// depth task "response scanning + observability + hardening", Part 1) —
// chatgpt.com, claude.ai, gemini.google.com; see content/response-scanner.js:
//
//   findLatestResponseTurn(): HTMLElement | null
//     Returns the DOM element containing the most recent AI response turn
//     (not the whole conversation history), or null if none is present
//     yet. Response-scanner.js re-queries this on every DOM mutation and
//     debounces until it stops changing — see that file's own doc comment
//     for the full design and its honestly-stated limitations.
//
// This split exists so a second site could be added later by writing one new
// adapter file plus one manifest content_scripts entry, without touching
// this file — which is exactly how claude.ai, gemini.google.com,
// chat.deepseek.com, and copilot.microsoft.com were added alongside the
// original chatgpt.com adapter. chatgpt.com is the only one of the five
// verified against a live, logged-in browser session — see
// extension/USER_GUIDE.md's caveats section and each new adapter file's own
// header comment for exactly what "unverified" means for that site
// specifically.
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
  // Response scanning ("response scanning + observability + hardening"
  // task, Part 1): the audit_log id of the most recent prompt scan that
  // actually sent something — read by content/response-scanner.js (loaded
  // alongside this file, sharing the same isolated-world scope) to
  // correlate a stabilised response back to the prompt that produced it.
  // See that file's own doc comment for the known limitation this simple,
  // single-slot approach has with rapid multi-prompt sessions.
  let lastScanId = null;

  function setLastScanId(id) {
    lastScanId = id;
  }

  function getLastScanId() {
    return lastScanId;
  }

  function init(adapter) {
    document.addEventListener("keydown", (e) => onKeydown(e, adapter), true);
    document.addEventListener("click", (e) => onClick(e, adapter), true);
    // Lets the popup confirm this content script is actually running on the
    // active tab (not just that the tab's URL matches a supported domain) —
    // see popup/popup.js's per-tab status check.
    chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
      if (message?.type === "LANGO_PING") {
        sendResponse({ siteName: adapter.siteName });
        return true;
      }
      return false;
    });
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
      let detail;
      if (response?.error === "not_authenticated") {
        detail = "not logged in — open the Lango extension options";
      } else if (response?.error === "consent_required") {
        // Server-side consent gate (routes/scan.rs) — distinct from a
        // plain auth/network failure so the message points at the right
        // fix (open the popup and accept the consent screen), not "log in
        // again" or "check your connection".
        detail = "consent required — open the Lango extension popup and accept the consent screen";
      } else {
        detail = response?.message || "Lango backend unreachable";
      }
      showBanner(`Lango: blocked — ${detail}. Prompt not sent.`, "blocked", { autoDismiss: false });
      return;
    }

    const result = response.data;
    // Response scanning (product-depth task, Part 1): every decision below
    // that actually sends the prompt records this scan's audit_log id so
    // content/response-scanner.js can correlate the AI's reply, once it
    // stabilises, back to this exact turn. `blocked_low_confidence` (below)
    // deliberately does NOT reach this line — nothing was sent, so there is
    // no response to ever correlate.
    if (result.decision !== "blocked_low_confidence") {
      LangoSiteAdapter.setLastScanId(result.id);
    }
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

      case "redacted_low_confidence_review": {
        // Handled the same as redacted_and_forwarded — the prompt was
        // genuinely redacted and is sent, not held back — but with a
        // visually distinct (amber) banner so the user can tell this
        // happened, even though nothing blocked them. No action is required
        // from the user here; this auto-dismisses like a normal redaction,
        // unlike the blocked case below.
        adapter.writeText(composer, result.redacted_prompt);
        showBanner(
          "Lango: redacted (low-confidence name match, flagged for review)",
          "reviewFlagged",
          { autoDismiss: true },
        );
        setTimeout(() => resend(adapter, composer), 60);
        break;
      }

      case "blocked_low_confidence":
        // Do NOT resend, do NOT auto-retry. The user must edit the prompt
        // themselves and submit again.
        //
        // Uses `result.user_message`, NOT `result.reason_string` — the
        // backend deliberately splits these two (see
        // backend/src/detection/plain_language.rs and
        // ScanOutcome/ScanResponse's own doc comments). `reason_string` is
        // full technical detail (entity_type names, confidence scores,
        // which specific detector/rule fired) meant for a compliance
        // officer reading the Audit Log later — showing it here, to the
        // person who just typed the prompt, used to leak exactly that
        // internal detail into this banner (e.g. "Scanner confidence below
        // threshold (0.50 < 0.60) on detected next_of_kin [capitalized-run
        // heuristic match, next-of-kin context], bank_account [primary
        // pattern match]"). `user_message` is the plain-language
        // counterpart built from the same match data, with none of that —
        // this is the one this banner should always show.
        showBanner(`Lango: blocked — ${result.user_message}`, "blocked", { autoDismiss: false });
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

  return { init, setLastScanId, getLastScanId };
})();

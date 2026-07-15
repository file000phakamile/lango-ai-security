// content/response-scanner.js — shared response-scanning orchestration
// ("response scanning + observability + hardening" task, Part 1). Loaded
// only for the three sites this task targets — chatgpt.com, claude.ai,
// gemini.google.com — NOT chat.deepseek.com or copilot.microsoft.com,
// which remain prompt-scanning only for now (out of scope here).
//
// WHY THIS IS A HARDER PROBLEM THAN PROMPT INTERCEPTION (stated plainly,
// per the task's explicit request): the prompt side reacts to one
// well-defined user action (pressing Enter or clicking Send) on an element
// whose *final* content is already known the instant that action happens.
// A response has no such moment — it streams into the DOM token by token
// (or chunk by chunk) over several seconds, so there is no single event
// that means "the response is finished." This module answers that with a
// debounce: watch for DOM mutations, and treat the response as complete
// once nothing has changed for `DEBOUNCE_MS`. This is a heuristic, not a
// guarantee — see the doc comment on `DEBOUNCE_MS` below for the real,
// measured data behind the current value, and the "Known fragility"
// section this task added to extension/README.md for what can still go
// wrong (a very long response with an unusually long pause mid-stream
// could be scanned prematurely; a slow network could make even 4s too
// short in rare cases).
//
// Correlating a response back to the prompt that produced it is also a
// real limitation, stated here rather than glossed over: this module only
// tracks the audit_log id of the MOST RECENT prompt scan that actually sent
// something (see `LangoSiteAdapter.setLastScanId`/`getLastScanId` in
// site-adapter.js). If a user sends a second prompt before the first
// response has stabilised and been scanned, the response scan for the
// first turn can end up attributed to the second prompt's audit_log row
// instead (or simply never fire, since the id it needed has already been
// overwritten). For the common case — one prompt, wait for the reply, then
// the next prompt — this works correctly; rapid multi-prompt sessions are
// the scenario where it doesn't, and that's an honest limitation, not
// something this module tries to fully solve.
const LangoResponseScanner = (() => {
  // Evidence-based, not a guess: a real, live test against gemini.google.com
  // (an anonymous session, no login required — see Questions.md for the
  // full writeup) measured actual streaming mutation gaps up to ~2.9
  // seconds for a 6-sentence response, meaning any debounce shorter than
  // that would have scanned a truncated mid-stream response in that exact,
  // real test. 4000ms keeps a comfortable margin above that measured
  // maximum. This has NOT been measured against chatgpt.com or claude.ai
  // (both blocked from this development environment by bot-detection
  // challenges — see extension/README.md) — their real streaming cadence
  // may differ, and this same constant is used for all three sites for now
  // since there's no verified site-specific data to tune it against.
  const DEBOUNCE_MS = 4000;

  const scannedElements = new WeakSet();
  let pendingElement = null;
  let debounceTimer = null;
  // Design pass, Step 5: covers the FULL user-perceived wait (still-
  // streaming detection + the debounce tail), not just the fast round trip
  // after the debounce fires — this is the specific case the design
  // direction called out by name ("particularly response scanning"), since
  // it's still ~8-9s even after the performance pass's Step 3 fix.
  let activeIndicator = null;

  function init(adapter) {
    if (typeof adapter.findLatestResponseTurn !== "function") return;
    const observer = new MutationObserver((mutations) => onMutation(adapter, mutations));
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
  }

  // Performance pass, Step 2/3: this used to reset the debounce timer on
  // ANY mutation anywhere in document.body — a suggestion chip fading in, a
  // "regenerate" button appearing, or any other unrelated page chrome
  // change would restart the full DEBOUNCE_MS wait just as much as new
  // response text would, even after the response itself had genuinely
  // finished rendering. That's a real, measured contributor to the 11-15s
  // real-world wait item 31 found (see Questions.md's Step 2 write-up) —
  // longer than "streaming time + one clean debounce tail" predicts. Fixed
  // by actually looking at the MutationRecords the observer already
  // receives (previously discarded) and only treating the response as
  // still-changing if at least one mutation's target is inside the
  // response element itself. DEBOUNCE_MS is NOT lowered — this makes the
  // "wait until the response has genuinely stopped changing" guarantee
  // more accurate, not weaker: it still waits the full measured-safe 4000ms
  // after the response itself last changed, it just stops being fooled by
  // unrelated page activity into waiting longer than that.
  function onMutation(adapter, mutations) {
    const latest = adapter.findLatestResponseTurn();
    if (!latest || scannedElements.has(latest)) return;

    const isNewResponseTurn = latest !== pendingElement;
    const mutationTouchesResponse =
      isNewResponseTurn || mutations.some((m) => latest.contains(m.target));
    if (!mutationTouchesResponse) return;

    if (isNewResponseTurn && !activeIndicator) {
      activeIndicator = startScanIndicator("Lango: checking response…", [
        "Lango: checking response for sensitive content…",
        "Lango: almost done reviewing the reply…",
      ]);
    }

    pendingElement = latest;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => onStable(), DEBOUNCE_MS);
  }

  async function onStable() {
    const el = pendingElement;
    // Claims the active indicator (if any) for this specific stabilisation
    // — every early `return` below routes through `finishSilently()` so the
    // indicator never gets stranded on screen if this function bails out
    // partway through.
    const indicator = activeIndicator;
    activeIndicator = null;
    function finishSilently() {
      if (indicator) indicator.clear();
    }

    if (!el || scannedElements.has(el)) {
      finishSilently();
      return;
    }
    scannedElements.add(el);

    const auditLogId = LangoSiteAdapter.getLastScanId();
    if (!auditLogId) {
      finishSilently(); // no correlated prompt scan this session — nothing to attach this response to
      return;
    }

    const text = (el.innerText != null ? el.innerText : el.textContent || "").trim();
    if (!text) {
      finishSilently();
      return;
    }

    // Real-latency instrumentation (performance pass, Step 1/3): measures
    // from the moment the debounce fires (the response was judged stable)
    // to the moment a decision comes back — the piece of the total
    // prompt-to-banner latency that's under the backend/network's control,
    // as distinct from the debounce wait itself (a deliberate, separately
    // reasoned client-side delay — see DEBOUNCE_MS's own comment above) and
    // from however long the AI provider itself took to finish streaming.
    const scanStartedAt = performance.now();
    let response;
    try {
      response = await chrome.runtime.sendMessage({
        type: "LANGO_SCAN_RESPONSE",
        auditLogId,
        responseText: text,
      });
    } catch (err) {
      response = null;
    }
    console.debug(`[Lango][perf] response scan round trip (debounce fired -> response received): ${Math.round(performance.now() - scanStartedAt)}ms`);

    // Fail OPEN here, deliberately, unlike the prompt side's fail-closed
    // rule: the response has already rendered and there is nothing left to
    // block — the user already saw it. A scan failure here just means no
    // warning banner appears, a degraded-but-safe outcome (the same
    // category as any other best-effort client-side check), not a security
    // gap the way a failed PROMPT scan would be. Logged to the console for
    // anyone debugging, not surfaced to the user as an error.
    if (!response || response.ok !== true) {
      console.warn("[Lango] response scan could not complete:", response);
      finishSilently();
      return;
    }

    if (response.data.flagged) {
      if (indicator) {
        indicator.done(`Lango: ${response.data.user_message}`, "reviewFlagged", { autoDismiss: false });
      } else {
        showBanner(`Lango: ${response.data.user_message}`, "reviewFlagged", { autoDismiss: false });
      }
    } else {
      // Not flagged: deliberately silent, no banner at all — a banner on
      // every single clean response would train the user to ignore Lango's
      // banners entirely, defeating the point of showing one when it matters.
      finishSilently();
    }
  }

  return { init };
})();

// content/deepseek-adapter.js — chat.deepseek.com-specific DOM hooks.
//
// *** STILL UNVERIFIED, and now confirmed UNVERIFIABLE from this specific
// dev environment by two independent methods, not just "never gotten
// around to it." *** Say this plainly rather than dressing it up:
//   1. A headless-Playwright navigation to chat.deepseek.com returns an
//      immediate HTTP 403 ("Request blocked") before any real page loads.
//   2. A plain `curl` (no browser, no JS engine — checked specifically to
//      rule out "it's just a headless-browser-fingerprint problem") gets an
//      HTTP 202 whose entire body is an AWS WAF ("Goku") JavaScript
//      challenge page — a real, active bot-verification gate that requires
//      executing and passing a browser-fingerprint check before the site
//      serves ANY real content, chat UI or otherwise, to either method.
// This is a materially different (and stronger) finding than the previous
// pass's "no display server to load a real extension in" — that blocker
// would, at least in principle, go away with a different environment; this
// one is the SITE itself actively refusing automated access regardless of
// how it's driven. Contrast with copilot.microsoft.com (see
// copilot-adapter.js), which a plain `curl` reached successfully and
// yielded real, checkable markup — DeepSeek's WAF specifically prevented
// the same technique from working here.
//
// Given that, this remains the LEAST confident of the five adapters in this
// extension, unchanged from the previous assessment: DeepSeek's web chat UI
// is not something this model has reliable, specific, current knowledge
// of — there is no well-documented public convention (comparable to
// chatgpt.com's long-stable `#prompt-textarea` id) to build on here. The
// selectors below are a genuine best-effort guess based on common patterns
// for a simple chat composer (a plain `<textarea>` is the most likely
// element type for an app of this kind, more likely than a rich-text editor
// like the other three), not a claim of specific knowledge about this site's
// actual markup. Treat this file as the first one to rewrite from scratch
// after checking chat.deepseek.com's real DOM directly (right-click the
// composer → Inspect, from a real logged-in session/browser, not
// automation) rather than the first one to trust.

const DeepSeekAdapter = {
  siteName: "chat.deepseek.com",

  findComposer() {
    const selectors = [
      "textarea#chat-input", // guessed id, unverified
      'textarea[placeholder*="Message" i]',
      'textarea[placeholder*="Send a message" i]',
      "form textarea",
      "textarea", // generic fallback
      'div[contenteditable="true"]', // in case the composer turns out to be contenteditable, not a textarea
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
  },

  findSendButton(composer) {
    const selectors = [
      'button[aria-label*="Send" i]',
      'div[role="button"][aria-label*="Send" i]', // some chat UIs use a styled <div role="button"> instead of a real <button>
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    const form = composer && composer.closest ? composer.closest("form") : null;
    return form ? form.querySelector('button[type="submit"]') : null;
  },

  readText(composer) {
    if (composer.tagName === "TEXTAREA" || composer.tagName === "INPUT") {
      return composer.value;
    }
    return composer.innerText != null ? composer.innerText : composer.textContent || "";
  },

  writeText(composer, text) {
    if (composer.tagName === "TEXTAREA" || composer.tagName === "INPUT") {
      // Same React-controlled-input native-setter technique as
      // chatgpt-adapter.js — applied defensively here since it's harmless if
      // the framework turns out not to need it (a plain, uncontrolled
      // textarea's `.value` assignment still works fine through the native
      // setter), but necessary if it does.
      const proto =
        composer.tagName === "TEXTAREA" ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
      const nativeSetter = Object.getOwnPropertyDescriptor(proto, "value").set;
      nativeSetter.call(composer, text);
      composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
      return;
    }

    // contenteditable fallback path, in case findComposer's last-resort
    // selector matched instead of a textarea.
    composer.focus();
    composer.textContent = text;
    composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    composer.dispatchEvent(new Event("change", { bubbles: true }));
  },
};

LangoSiteAdapter.init(DeepSeekAdapter);

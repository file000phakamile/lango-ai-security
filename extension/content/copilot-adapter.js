// content/copilot-adapter.js — copilot.microsoft.com-specific DOM hooks.
//
// This is Microsoft's CONSUMER web chat at copilot.microsoft.com
// specifically — NOT GitHub Copilot (a different product, mostly
// IDE-embedded, out of scope for this browser extension entirely) and NOT
// Copilot embedded inside Office/Microsoft 365 apps (also out of scope —
// see this adapter's own limitation and the Known Limitations note in
// extension/USER_GUIDE.md about AI features embedded inside other products).
//
// *** COMPOSER SELECTOR CONFIRMED AGAINST LIVE MARKUP — genuinely verified,
// not a guess, though NOT via loading the real extension. *** Loading this
// extension as a real browser extension is still not possible in this dev
// environment (no display server — Playwright/Chromium never registers an
// extension service worker regardless of headless mode, re-confirmed
// directly, not assumed, when this verification pass started). But
// copilot.microsoft.com itself, unlike chat.deepseek.com (see
// deepseek-adapter.js), is reachable with a plain HTTP fetch — a direct
// `curl` of `https://copilot.microsoft.com/` (real request, real response,
// checked into this repo's history via Questions.md, not hypothetical)
// returned the server-rendered initial HTML containing:
//   <textarea id="userInput" data-testid="composer-input" ... placeholder="Message Copilot">
// — a plain `<textarea>`, NOT a contenteditable rich-text editor, with BOTH
// the historically-guessed `id="userInput"` AND a more specific
// `data-testid="composer-input"` attribute, confirmed present TODAY, not
// just "historically documented." `findComposer` below is now genuinely
// verified for this specific site, at least as of this check.
//
// The send button is NOT confirmed the same way: it didn't appear anywhere
// in that same static HTML pull (only 6 `<button>` elements were present at
// all — attach-file, sidebar toggle, library — no send button), which is
// consistent with a send button that only mounts once the composer has
// text in it, not evidence the guessed selectors below are wrong, just that
// this method couldn't confirm or refute them either way. This matters less
// than it would elsewhere: `site-adapter.js`'s Enter-key interception path
// only needs `findComposer` to fire `handleSubmitAttempt` at all — a wrong
// or missing `findSendButton` result only affects the `resend()` step
// afterward, which already falls back to a synthetic Enter keydown on the
// composer if no usable send button is found (see `site-adapter.js`'s
// `resend`), so this adapter still has a real, load-bearing path to
// working even if every one of these send-button guesses is wrong.

const CopilotAdapter = {
  siteName: "copilot.microsoft.com",

  findComposer() {
    const selectors = [
      'textarea[data-testid="composer-input"]', // CONFIRMED against a live fetch of copilot.microsoft.com — see header comment
      "textarea#userInput", // also confirmed present today (same element — belt-and-suspenders in case the testid changes first)
      'textarea[data-testid="chat-input-textarea"]',
      'textarea[aria-label*="Ask me anything" i]',
      'textarea[aria-label*="message" i]',
      "form textarea",
      "textarea", // generic fallback
      'div[contenteditable="true"]', // in case the composer has since moved to a contenteditable rich-text editor
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
  },

  findSendButton(composer) {
    // UNVERIFIED — see header comment: the send button did not appear in
    // the static HTML this was checked against, likely because it only
    // mounts once the composer has text. Kept as a best-effort guess; the
    // Enter-key path (see site-adapter.js) does not depend on this working.
    const selectors = [
      'button[data-testid="chat-input-send-button"]',
      'button[aria-label="Submit"]',
      'button[aria-label*="Send" i]',
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
      // chatgpt-adapter.js, applied defensively — Microsoft's consumer web
      // apps are commonly React-based, so a plain `.value =` assignment is
      // likely (not confirmed) to be silently ignored the same way it is on
      // chatgpt.com.
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

LangoSiteAdapter.init(CopilotAdapter);

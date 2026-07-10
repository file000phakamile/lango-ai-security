// content/claude-adapter.js — claude.ai-specific DOM hooks.
//
// *** UNVERIFIED — same caveat as chatgpt-adapter.js, read this before
// trusting it. *** This adapter has never been loaded as a real extension
// against a live, logged-in claude.ai session — the same environment
// blockers documented in Questions.md (no display server for a real
// Chromium extension context, and no Anthropic account available even if
// that were solved) apply here too, not just to chatgpt.com. Selectors below
// are a best-effort, defensively-ordered list (most-specific/most-stable
// candidate first, generic fallback last) based on claude.ai's publicly
// documented UI patterns as of this writing — moderate confidence, not
// verified. claude.ai's composer has historically been a ProseMirror-based
// contenteditable rich-text editor, structurally similar to chatgpt.com's —
// see writeText's comment for why that similarity matters and where it
// might not hold.

const ClaudeAdapter = {
  siteName: "claude.ai",

  findComposer() {
    const selectors = [
      'div[contenteditable="true"][aria-label="Write your prompt to Claude"]', // most specific known hook, if the aria-label text hasn't changed
      "div.ProseMirror[contenteditable=\"true\"]", // claude.ai's composer has historically been ProseMirror-based, same editor family as chatgpt.com's
      'fieldset div[contenteditable="true"]', // the composer has historically sat inside a <fieldset> wrapping the whole input area
      'div[contenteditable="true"][id]', // generic fallback, requiring an id to avoid matching an unrelated contenteditable region on the page
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
  },

  findSendButton(composer) {
    const selectors = [
      'button[aria-label="Send Message"]', // historically Claude's label, exact casing included since aria-label matches are case-sensitive
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
    return composer.innerText != null ? composer.innerText : composer.textContent || "";
  },

  writeText(composer, text) {
    // No React-controlled-input <textarea>/<input> value setter needed here
    // — claude.ai's composer is (as far as could be determined without live
    // verification) a contenteditable div, not a plain form control, so
    // there is no native `.value` property for React to shadow in the first
    // place. The same caveat chatgpt-adapter.js documents for its own
    // contenteditable path applies identically: setting `.textContent`
    // directly bypasses ProseMirror's own transaction/state system, so it
    // may visually update the composer without Claude's internal editor
    // state agreeing, in which case the resend could send stale or empty
    // content. This is the single most likely point of failure in this
    // adapter if it doesn't work.
    composer.focus();
    composer.textContent = text;
    composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    composer.dispatchEvent(new Event("change", { bubbles: true }));
  },
};

LangoSiteAdapter.init(ClaudeAdapter);

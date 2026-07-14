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
//
// Re-checked during the response-scanning task ("response scanning +
// observability + hardening"): both a headless-browser navigation and a
// raw, unauthenticated HTTP fetch of claude.ai were attempted again, and
// both are still fully blocked (claude.ai redirects to /login and returns
// HTTP 403 even to a plain curl-style request, unlike copilot.microsoft.com
// or chatgpt.com, where the raw-HTTP path got through even though the
// browser path didn't — see Questions.md for the exact results). There is
// still no way to verify anything in this file against real markup.
//
// RESPONSE SCANNING SPECIFICALLY (Part 1 of that task) IS LOWER CONFIDENCE
// THAN THE COMPOSER SELECTORS ABOVE, stated plainly: unlike chatgpt.com's
// `data-message-author-role` attribute (a widely and consistently
// documented convention across several years of public tooling),
// claude.ai's response-turn markup has no comparably well-established
// public convention this could be based on — `findLatestResponseTurn`
// below is a lower-confidence guess than everything else in this file, not
// just "the same unverified status as the rest."

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

  // Response scanning (product-depth task, Part 1) — see file header:
  // lower confidence than every other selector in this file. `[data-
  // testid="conversation-turn"]` has been referenced in public discussion
  // of claude.ai's DOM as marking each turn (both user and assistant);
  // `.font-claude-message` has similarly been referenced as specific to
  // assistant turns specifically. Tried in that order — the more specific,
  // assistant-only class first — with the turn-level selector as a
  // fallback that (if it matches BOTH user and assistant turns, which is
  // genuinely unclear without live verification) could return a user turn
  // as "the latest response," which would silently scan the wrong text.
  // This is the single most likely part of this adapter to be simply wrong.
  findLatestResponseTurn() {
    const selectors = [".font-claude-message", '[data-testid="conversation-turn"]'];
    for (const sel of selectors) {
      const turns = document.querySelectorAll(sel);
      if (turns.length) return turns[turns.length - 1];
    }
    return null;
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
LangoResponseScanner.init(ClaudeAdapter);

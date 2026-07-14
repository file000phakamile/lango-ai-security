// content/gemini-adapter.js — gemini.google.com-specific DOM hooks.
//
// *** COMPOSER + RESPONSE SELECTORS NOW VERIFIED — see below for exactly
// what was and wasn't checked. *** Earlier revisions of this file were
// entirely unverified, same as every other adapter in this pass; that
// changed for this site specifically during the response-scanning task
// ("response scanning + observability + hardening"), when
// gemini.google.com turned out to be reachable and — unexpectedly — usable
// WITHOUT logging into a Google account at all (an anonymous session, real
// production gemini.google.com, real model replies). Full findings in
// Questions.md; summary here:
//
//   - The Shadow DOM risk this file used to warn about did NOT materialise:
//     `document.querySelector`/`querySelectorAll` see straight through to
//     the real composer and response elements with no special handling
//     needed. This was the single biggest documented uncertainty in this
//     adapter and it's now resolved, not just assumed away.
//   - The real composer is `rich-textarea .ql-editor[contenteditable="true"]`
//     — confirmed via live DOM inspection (Quill-editor-style, exactly as
//     guessed) — with the real `aria-label` text corrected below to what
//     was actually observed ("Enter a prompt for Gemini", not "Enter a
//     prompt here" as an earlier revision guessed).
//   - The real response element is `message-content` (a custom element),
//     confirmed to hold ONLY the clean response text (no "Gemini said"
//     label noise, unlike its `model-response` ancestor) and confirmed, in
//     a real two-message-turn session, to return turns via
//     `querySelectorAll` in correct chronological DOM order — the last
//     match is reliably the most recent turn.
//   - What was NOT verified: a real, logged-in Google account session
//     (only anonymous access was tested — a logged-in session's DOM could
//     genuinely differ), the send-button selectors below (Enter-key submit
//     was used throughout testing, not a button click), and either of the
//     other two target sites (chatgpt.com and claude.ai both remain fully
//     blocked from this development environment — see extension/README.md).
//     Treat findSendButton and writeText's contenteditable-write path as
//     still unverified, same caveat level as before.

const GeminiAdapter = {
  siteName: "gemini.google.com",

  findComposer() {
    const selectors = [
      "rich-textarea .ql-editor[contenteditable=\"true\"]", // VERIFIED — see file header
      'div[aria-label="Enter a prompt for Gemini"][contenteditable="true"]', // VERIFIED real aria-label text, kept as a second, more specific matcher
      '.ql-editor[contenteditable="true"]',
      'div[contenteditable="true"][role="textbox"]', // generic fallback
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
  },

  // Response scanning (product-depth task, Part 1) — VERIFIED against a
  // real anonymous gemini.google.com session (see file header). Returns the
  // most recent assistant turn's clean text content, or null if none exists
  // yet (e.g. no message has been sent this page load).
  findLatestResponseTurn() {
    const turns = document.querySelectorAll("message-content");
    return turns.length ? turns[turns.length - 1] : null;
  },

  findSendButton(composer) {
    const selectors = [
      'button[aria-label="Send message"]', // historically Gemini's label
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
    // Contenteditable path only — no plain <textarea>/<input> is expected
    // here, so no React/Angular-controlled-input value setter is needed.
    // Quill-based editors (if that's genuinely what's rendered here, itself
    // unverified) maintain their own internal document model separately
    // from the DOM, same category of risk as ProseMirror in
    // chatgpt-adapter.js/claude-adapter.js: setting `.textContent` directly
    // may visually update the composer without Gemini's own editor state
    // agreeing, risking a resend of stale or empty content.
    composer.focus();
    composer.textContent = text;
    composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    composer.dispatchEvent(new Event("change", { bubbles: true }));
  },
};

LangoSiteAdapter.init(GeminiAdapter);
LangoResponseScanner.init(GeminiAdapter);

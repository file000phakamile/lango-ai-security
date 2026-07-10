// content/gemini-adapter.js — gemini.google.com-specific DOM hooks.
//
// *** UNVERIFIED, and lower confidence than chatgpt-adapter.js or
// claude-adapter.js — read this before trusting it. *** Never loaded against
// a live, logged-in gemini.google.com session, same environment blockers as
// every other adapter in this pass (see Questions.md). But there is a
// second, more specific reason to distrust this one: Gemini's composer has
// historically been built on a custom element (`<rich-textarea>` wrapping a
// Quill-editor-style `.ql-editor` contenteditable div), and Google's web
// products frequently use Shadow DOM to encapsulate custom elements like
// this. If gemini.google.com's `<rich-textarea>` uses a **closed** shadow
// root, `document.querySelector` cannot see inside it AT ALL — every
// selector below would return null, `findComposer` would return null, and
// this adapter would do nothing (no interception, no banner, Gemini behaves
// as if the extension weren't installed), the same clean-failure mode
// documented for every adapter's "site changed its UI" case, just
// potentially true from day one here rather than only after a future UI
// change. This was not possible to check without a live session. If this
// adapter appears to do nothing at all on gemini.google.com, an open (not
// closed) shadow root — or a `document.querySelector` rewritten to
// pierce a specific known open shadow host — is the first thing to check,
// not a typo in the selectors below.

const GeminiAdapter = {
  siteName: "gemini.google.com",

  findComposer() {
    const selectors = [
      'div[aria-label="Enter a prompt here"][contenteditable="true"]', // historically Gemini's placeholder/aria-label text
      "rich-textarea .ql-editor[contenteditable=\"true\"]", // Gemini's composer has historically been a Quill-editor-style contenteditable inside a <rich-textarea> custom element — see file header re: Shadow DOM risk
      '.ql-editor[contenteditable="true"]',
      'div[contenteditable="true"][role="textbox"]', // generic fallback
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
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

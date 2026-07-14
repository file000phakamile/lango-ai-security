// content/chatgpt-adapter.js — chatgpt.com-specific DOM hooks.
//
// *** THIS IS THE SINGLE MOST LIKELY FILE TO BREAK if OpenAI changes their
// UI. *** Selectors below are a best-effort, defensively-ordered list
// (most-specific/most-stable candidate first, generic fallback last) based
// on publicly documented chatgpt.com UI patterns — NOT verified against a
// live, logged-in chatgpt.com session. A real, unauthenticated raw HTTP
// fetch of chatgpt.com during the response-scanning task ("response
// scanning + observability + hardening") did succeed (HTTP 200, unlike a
// headless-browser navigation, which still gets a Cloudflare 403 — the
// same server-vs-client-rendering gap documented for copilot.microsoft.com
// elsewhere in this project) and confirmed `#prompt-textarea` is still the
// real composer id on the current landing-page shell — but that page has
// no conversation on it (it's the logged-out shell), so it could not
// confirm anything about response-turn markup, which only exists once a
// real, authenticated conversation has messages in it. `findSendButton` and
// the response-scanning selectors below (`findLatestResponseTurn`) remain
// exactly as unverified as before this check. Treat this file as
// unverified until someone runs it in a real, logged-in browser — see the
// manual testing steps in extension/README.md.
//
// RESPONSE SCANNING (Part 1 of that task) IS A HARDER, LESS-VERIFIABLE
// PROBLEM THAN THE COMPOSER SIDE, stated plainly: `data-message-author-
// role="assistant"` is a widely and consistently documented ChatGPT DOM
// convention across numerous public userscripts/extensions over several
// years (unlike some other selectors here, which are closer to educated
// single-source guesses) — moderate-to-reasonable confidence as selectors
// go, but still genuinely unverified against a live session, and response
// markup changes more often in practice across ChatGPT UI revisions than
// the composer has.

const ChatGptAdapter = {
  siteName: "chatgpt.com",

  findComposer() {
    const selectors = [
      "#prompt-textarea", // most stable known hook — this id has persisted across chatgpt.com UI rewrites, even when the underlying element type changed from <textarea> to a contenteditable rich-text editor
      'div[contenteditable="true"][id]',
      "form textarea",
      "textarea",
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    return null;
  },

  findSendButton(composer) {
    const selectors = [
      'button[data-testid="send-button"]', // OpenAI has historically used data-testid on this button specifically
      'button[aria-label="Send prompt"]',
      'button[aria-label*="Send" i]',
    ];
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) return el;
    }
    // Last resort: the nearest form's submit button.
    const form = composer && composer.closest ? composer.closest("form") : null;
    return form ? form.querySelector('button[type="submit"]') : null;
  },

  // Response scanning (product-depth task, Part 1) — see file header for
  // the confidence level on this specific selector (moderate: a widely
  // documented convention, still unverified live). Each ChatGPT turn is
  // marked with `data-message-author-role="user"` or `"assistant"`; the
  // last assistant-role element in document order is the most recent reply.
  findLatestResponseTurn() {
    const turns = document.querySelectorAll('[data-message-author-role="assistant"]');
    return turns.length ? turns[turns.length - 1] : null;
  },

  readText(composer) {
    if (composer.tagName === "TEXTAREA" || composer.tagName === "INPUT") {
      return composer.value;
    }
    // contenteditable path
    return composer.innerText != null ? composer.innerText : composer.textContent || "";
  },

  writeText(composer, text) {
    if (composer.tagName === "TEXTAREA" || composer.tagName === "INPUT") {
      // React-controlled inputs ignore a plain `el.value = text` assignment
      // because React tracks value changes through its own synthetic event
      // system, not through the DOM property directly — the setter React
      // itself installed on the instance shadows the prototype's setter, so
      // writing `.value` normally never reaches React's internal state. The
      // fix: grab the *native* HTMLTextAreaElement/HTMLInputElement
      // prototype's value setter directly (bypassing React's instance-level
      // override) and call it explicitly, then dispatch a real InputEvent
      // so React's change-detection (which listens for native `input`
      // events) picks it up.
      const proto =
        composer.tagName === "TEXTAREA" ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
      const nativeSetter = Object.getOwnPropertyDescriptor(proto, "value").set;
      nativeSetter.call(composer, text);
      composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
      return;
    }

    // contenteditable path — chatgpt.com's composer is, as of the last
    // publicly documented UI structure, a ProseMirror-based rich-text editor
    // rather than a plain textarea, NOT independently verified live here
    // (see the file header). This is the least reliable part of this
    // adapter: directly setting .textContent bypasses ProseMirror's own
    // transaction/state system entirely, so it may visually update the
    // composer without ChatGPT's internal editor state agreeing — in which
    // case the resend could send stale or empty content. If redaction
    // visually "works" but the wrong text gets sent, this is the first
    // place to look.
    composer.focus();
    composer.textContent = text;
    composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
    composer.dispatchEvent(new Event("change", { bubbles: true }));
  },
};

LangoSiteAdapter.init(ChatGptAdapter);
LangoResponseScanner.init(ChatGptAdapter);

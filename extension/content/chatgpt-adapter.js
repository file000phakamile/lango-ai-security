// content/chatgpt-adapter.js — chatgpt.com-specific DOM hooks.
//
// *** THIS IS THE SINGLE MOST LIKELY FILE TO BREAK if OpenAI changes their
// UI. *** Selectors below are a best-effort, defensively-ordered list
// (most-specific/most-stable candidate first, generic fallback last) based
// on publicly documented chatgpt.com UI patterns — NOT verified against a
// live, logged-in chatgpt.com session. Every attempt to reach chatgpt.com
// from this development environment (headless and non-headless) was stopped
// by a Cloudflare bot-check challenge before the real app ever loaded, and
// there is no OpenAI account available to log in with regardless. See
// Questions.md for exactly what was tried. Treat this file as unverified
// until someone runs it in a real, logged-in browser — see the manual
// testing steps in extension/README.md.

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

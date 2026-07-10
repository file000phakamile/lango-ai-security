// content/deepseek-adapter.js — chat.deepseek.com-specific DOM hooks.
//
// *** UNVERIFIED, and the LEAST confident of the four new adapters added in
// this pass — say this plainly rather than dressing it up. *** Never loaded
// against a live, logged-in chat.deepseek.com session, same environment
// blockers as every other adapter here (see Questions.md). Unlike
// chatgpt.com, claude.ai, and even gemini.google.com, DeepSeek's web chat UI
// is not something this model has reliable, specific, current knowledge
// of — there is no well-documented public convention (comparable to
// chatgpt.com's long-stable `#prompt-textarea` id) to build on here. The
// selectors below are a genuine best-effort guess based on common patterns
// for a simple chat composer (a plain `<textarea>` is the most likely
// element type for a app of this kind, more likely than a rich-text editor
// like the other three), not a claim of specific knowledge about this site's
// actual markup. Treat this file as the first one to rewrite from scratch
// after checking chat.deepseek.com's real DOM directly (right-click the
// composer → Inspect) rather than the first one to trust.

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

// content/copilot-adapter.js — copilot.microsoft.com-specific DOM hooks.
//
// This is Microsoft's CONSUMER web chat at copilot.microsoft.com
// specifically — NOT GitHub Copilot (a different product, mostly
// IDE-embedded, out of scope for this browser extension entirely) and NOT
// Copilot embedded inside Office/Microsoft 365 apps (also out of scope —
// see this adapter's own limitation and the Known Limitations note in
// extension/USER_GUIDE.md about AI features embedded inside other products).
//
// *** UNVERIFIED — same caveat as every other adapter added in this pass.
// *** Never loaded against a live copilot.microsoft.com session, same
// environment blockers documented in Questions.md. Moderate confidence:
// copilot.microsoft.com evolved from Bing Chat's consumer web interface,
// which historically used a plain `<textarea>` composer (an `id="searchbox"`
// / `id="userInput"`-style hook, depending on which product era) rather than
// a rich-text contenteditable editor the way chatgpt.com/claude.ai/Gemini
// do — but this has not been confirmed against the current, live
// copilot.microsoft.com UI, which may well have changed since.

const CopilotAdapter = {
  siteName: "copilot.microsoft.com",

  findComposer() {
    const selectors = [
      "textarea#userInput", // historical Bing Chat/Copilot composer id — unverified against the current site
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

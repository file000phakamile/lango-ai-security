# Lango Browser Extension (v0.1)

A Manifest V3 Chrome/Edge extension that puts Lango's redaction layer directly in
front of ChatGPT's web UI: before a prompt leaves your browser, it's scanned by the
real Lango backend (the same `/api/scan` endpoint documented in the main repo README),
and redacted or blocked according to the same fail-closed logic used everywhere else
in this project.

## Scope: chatgpt.com only, v0.1

This extension supports **chatgpt.com only**. It is not a multi-site tool, and it does
not claim to be. The code is structured with a site-adapter interface
(`content/site-adapter.js` defines the contract; `content/chatgpt-adapter.js`
implements it for chatgpt.com) specifically so a second site — claude.ai,
gemini.google.com — could be added later as one new adapter file plus one new
`content_scripts` entry in `manifest.json`, without touching the shared interception
logic. But "structured to make it easier later" is not the same as "supported now":
no other site is implemented, tested, or claimed to work. See Questions.md in the repo
root for the specific reasoning behind stopping at one site for this pass.

## What it does

1. You type a prompt into ChatGPT and hit Enter or click Send.
2. The extension intercepts that submit **before** it reaches ChatGPT's own send
   logic, reads the prompt text, and sends it to the Lango backend's `/api/scan`
   endpoint (via the background service worker — see Architecture below).
3. Depending on the response:
   - **`cleared_no_entities`** — no sensitive entities found. The extension completes
     the send itself, with your original text unchanged.
   - **`redacted_and_forwarded`** — sensitive entities were found and redacted. The
     extension replaces the composer's text with the redacted version, then sends
     *that*.
   - **`blocked_low_confidence`** — the scanner found something it isn't confident
     enough about to safely redact. Nothing is sent. You get a banner explaining why
     (the backend's `reason_string`) and have to edit the prompt yourself and resubmit
     — the extension does not retry automatically.
   - **Any error** (network failure, Lango backend unreachable, expired login) — fails
     **closed**: nothing is sent, and you get a clear "Lango is unreachable, prompt
     not sent" banner. This matches the fail-closed principle used everywhere else in
     this project (see `docs/ARCHITECTURE.md`, `docs/SECURITY_PRIVACY.md`) — an
     unscanned prompt must never slip through just because the scanner itself was
     unreachable.
4. A small banner near the bottom of the page shows what happened, colour-matched to
   the dashboard (gold `#8A6323` = redacted, red `#A83A3A` = blocked, green `#2F7A53`
   = cleared). Non-blocking outcomes auto-dismiss after a few seconds; blocked/error
   banners stay until you take action.

## Architecture

```
extension/
  manifest.json            Manifest V3 config — permissions, content script matches
  background.js            Service worker: owns the JWT, does the actual fetch()
                            calls to the Lango API (content scripts can't make
                            cross-origin fetch() calls directly under MV3 — see the
                            comment at the top of background.js)
  content/
    ui-banner.js            Shared banner/toast UI, used by any site adapter
    site-adapter.js          Shared interception orchestration (document-level
                            capture-phase listeners, decision handling, resend
                            logic) — site-independent
    chatgpt-adapter.js       chatgpt.com-specific DOM hooks — see Known fragility
  popup/
    popup.html / popup.js   Extension icon popup: connection status, session scan
                            count, link to the live dashboard
  options/
    options.html / options.js   One-time login form against the real backend's
                            POST /api/auth/login; stores the JWT in
                            chrome.storage.local; lets you override the API base URL
  icons/                   Placeholder gold-shield icons (16/48/128px) — functional,
                            not designed, per this pass's explicit scope
```

The background service worker is the only place that talks to the network. Content
scripts message it via `chrome.runtime.sendMessage` and get a plain
`{ ok: true, data: {...} }` or `{ ok: false, error: "...", message: "..." }` response
back — this keeps every network/auth failure mode centralized in one file
(`background.js`) instead of duplicated across every future site adapter.

## Install and use (manual — see Verification below for why this is manual)

1. Open `chrome://extensions` (or `edge://extensions` on Edge).
2. Enable **Developer mode** (top-right toggle).
3. Click **Load unpacked**, and select this `extension/` directory.
4. Click the Lango icon in your toolbar, then **Open options / log in**.
5. Log in with a real Lango account (e.g. the seeded demo account —
   `compliance@lango.demo` / `LangoDemo123!`, see the main repo README) against the
   live backend (`https://lango-backend-qwkx.onrender.com`, prefilled by default), or
   override the API base URL to `http://localhost:8080` if you're running the backend
   locally (see the main README's Setup section).
6. Go to [chatgpt.com](https://chatgpt.com), type a prompt containing something
   sensitive-looking (e.g. a made-up national ID or phone number), and hit Enter.
   You should see a "Lango: scanning…" banner, followed by either the prompt being
   redacted-and-sent, blocked, or sent through unchanged, depending on what it found.

## Verification — what was and wasn't actually tested

Read this section before trusting the chatgpt.com-specific part of this extension.

**Verified, directly, against the live production backend** (not simulated): the
actual `background.js` file's `login()` and `scanPrompt()` functions were loaded
unmodified into a Node.js `vm` context with a minimal in-memory mock of
`chrome.storage.local`, and called against `https://lango-backend-qwkx.onrender.com`
for real. This confirms the extension's API wiring — the part independent of
chatgpt.com's DOM — actually works:

- Login with wrong credentials → rejected.
- Login with the real seeded demo credentials → succeeds, JWT stored.
- `/api/scan` called with that JWT → real detection response for all three decisions
  (`cleared_no_entities`, `redacted_and_forwarded`, `blocked_low_confidence`), matching
  the same behavior verified elsewhere in this project via `curl`.
- `/api/scan` called with no stored JWT → fails closed (`not_authenticated`), does not
  silently proceed.
- `/api/scan` called against an unreachable host → fails closed (`network_error`),
  does not silently proceed.

**NOT verified**: the extension was never loaded as a real browser extension and
driven against a live chatgpt.com page. Two independent things blocked this, tried in
this order:

1. **chatgpt.com itself is unreachable from this environment.** Every attempt (both
   headless and a forced non-headless launch) to load `https://chatgpt.com` was
   stopped by a Cloudflare bot-check interstitial ("Just a moment...") before the real
   app ever loaded — and there is no OpenAI account available to log in with even if
   that were bypassed.
2. **Loading the unpacked extension itself failed in this sandbox, separately from
   issue 1.** Playwright's default headless mode uses a stripped-down "headless
   shell" Chromium build that does not support extensions at all — no service worker
   ever registered, confirmed via direct CDP `Target.getTargets` polling (empty every
   time). Forcing the full Chromium binary via `headless: false` + Chrome's own
   `--headless=new` flag (the documented workaround) launched successfully but still
   registered no extension target — this environment has no display server, and that
   combination did not resolve it either. See Questions.md for the full trail.

**What this means practically**: the DOM-interception logic in
`content/chatgpt-adapter.js` — finding the composer, finding the send button, reading
and writing its text — is written from best-effort knowledge of chatgpt.com's
publicly documented UI patterns, but has not been confirmed against a live, logged-in
chatgpt.com session by anyone. **You should verify this yourself** using the manual
steps above before relying on it. If it doesn't work, the file's own comments point at
exactly which selectors to check first (`content/chatgpt-adapter.js`'s `findComposer`
and `findSendButton`), and at the specific contenteditable-vs-textarea uncertainty in
`writeText`.

## Known fragility

This is a standing limitation of the DOM-interception approach itself, not a bug to
be fixed later:

- **This extension depends entirely on chatgpt.com's current UI structure.** If
  OpenAI changes their composer's markup, id/data-testid attributes, or submit
  mechanism, `content/chatgpt-adapter.js`'s selectors can silently stop matching
  anything — at which point the extension does nothing (no interception, no banner,
  ChatGPT behaves as if the extension weren't installed) rather than failing loudly.
  This is the single most likely thing to break, and it can happen at any time without
  warning, entirely outside this project's control.
- **The composer's exact element type is a genuine unknown as of this writing** (see
  Verification above) — chatgpt.com's input has historically been a plain
  `<textarea>` in some versions and a ProseMirror-based `contenteditable` rich-text
  editor in others. `chatgpt-adapter.js`'s `writeText` handles both cases, but the
  `contenteditable` path is explicitly the less reliable of the two: it sets
  `.textContent` directly, which bypasses ProseMirror's own transaction/state system
  and may not reliably sync with what ChatGPT's own React state believes the composer
  contains.
- **A stable-looking selector (an `id` or `data-testid`) is not a guarantee.** These
  are conventions OpenAI has followed reasonably consistently, not a public API
  contract they've committed to.
- **No automated test suite covers `content/chatgpt-adapter.js`** for exactly this
  reason — there's nothing meaningful to assert against without a real, live
  chatgpt.com DOM to run against. `background.js`'s logic is the part that was
  actually verified (see above).

## Local development

To test against a locally-run backend instead of the live Render one: run the backend
locally (see the main repo README's "Full stack" setup section), then in the
extension's options page set **API base URL** to `http://localhost:8080` and log in
with the seed script's demo credentials. `http://localhost:8080/*` is already declared
in `manifest.json`'s `host_permissions`, so no manifest edits are needed for this.

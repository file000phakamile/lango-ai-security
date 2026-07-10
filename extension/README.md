# Lango Browser Extension (v0.1)

A Manifest V3 Chrome/Edge extension that puts Lango's redaction layer directly in
front of an AI chat site's web UI: before a prompt leaves your browser, it's scanned by
the real Lango backend (the same `/api/scan` endpoint documented in the main repo
README), and redacted or blocked according to the same fail-closed logic used
everywhere else in this project.

**New here? Read [USER_GUIDE.md](USER_GUIDE.md) instead** — a plain-language
install-and-use walkthrough for a first-time user (judge, mentor, tester), rather than
this file's developer-facing architecture/verification detail.

## Scope: five sites, one verified

This extension implements five sites:

| Site | Status |
|---|---|
| chatgpt.com | **Verified working** — driven against a real, logged-in browser session (see Verification below) |
| claude.ai | Implemented, **not yet verified** against a live page |
| gemini.google.com | Implemented, **not yet verified** against a live page |
| chat.deepseek.com | Implemented, **not yet verified** against a live page |
| copilot.microsoft.com (consumer web chat — not GitHub Copilot) | Implemented, **not yet verified** against a live page |

The code is structured with a site-adapter interface (`content/site-adapter.js`
defines the contract; each `content/<site>-adapter.js` implements it for one site)
specifically so a new site can be added as one new adapter file plus one new
`content_scripts` entry in `manifest.json`, without touching the shared interception
logic — this is exactly how the four newer adapters were added alongside the original
chatgpt.com one, without changing `site-adapter.js`'s orchestration logic at all (one
addition: a `LANGO_PING` listener the popup uses to confirm a content script is
actually running on the active tab).

**"Implemented" is not the same as "confirmed working."** Only chatgpt.com has
actually been driven against a live, logged-in browser session. The other four were
built using the same best-effort, defensively-ordered selector strategy that worked
for chatgpt.com, but for the same environment reasons documented below (no display
server, Playwright's Chromium doesn't support loading real extensions), none of them
could be verified here. See each adapter file's own header comment for a
site-specific honest confidence assessment — they are not all equally uncertain (see
[USER_GUIDE.md](USER_GUIDE.md)'s Caveats section and `Questions.md` for the summary).

## What it does

1. You type a prompt into a supported site's chat composer and hit Enter or click
   Send.
2. The extension intercepts that submit **before** it reaches the site's own send
   logic, reads the prompt text, and sends it to the Lango backend's `/api/scan`
   endpoint (via the background service worker — see Architecture below).
3. Depending on the response:
   - **`cleared_no_entities`** — no sensitive entities found. The extension completes
     the send itself, with your original text unchanged.
   - **`redacted_and_forwarded`** — sensitive entities were found with high enough
     confidence to redact automatically. The extension replaces the composer's text
     with the redacted version, then sends *that*.
   - **`redacted_low_confidence_review`** — a low-but-real-confidence `full_name`
     match (see `docs/SECURITY_PRIVACY.md`'s Human oversight row for the reasoning).
     Redacted and sent the same as above, but flagged with a visually distinct amber
     banner — the prompt is *not* held back, but the event is queryable separately in
     the audit log for asynchronous compliance review.
   - **`blocked_low_confidence`** — the scanner found something it isn't confident
     enough about to safely redact (near-zero confidence, or a low-confidence
     *structured* entity type). Nothing is sent. You get a banner explaining why
     (the backend's `reason_string`) and have to edit the prompt yourself and resubmit
     — the extension does not retry automatically.
   - **Any error** (network failure, Lango backend unreachable, expired login) — fails
     **closed**: nothing is sent, and you get a clear "Lango is unreachable, prompt
     not sent" banner. This matches the fail-closed principle used everywhere else in
     this project (see `docs/ARCHITECTURE.md`, `docs/SECURITY_PRIVACY.md`) — an
     unscanned prompt must never slip through just because the scanner itself was
     unreachable.
4. A small banner near the bottom of the page shows what happened, colour-matched to
   the dashboard (gold `#8A6323` = redacted, amber `#C2660C` = redacted-but-flagged-
   for-review, red `#A83A3A` = blocked, green `#2F7A53` = cleared). Non-blocking
   outcomes auto-dismiss after a few seconds; blocked/error banners stay until you
   take action.

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
                            logic, and a LANGO_PING responder the popup uses to
                            confirm a content script is actually running on the
                            active tab) — site-independent
    chatgpt-adapter.js       chatgpt.com-specific DOM hooks — VERIFIED, see
                            Verification below
    claude-adapter.js        claude.ai-specific DOM hooks — UNVERIFIED, see
                            Known fragility and the file's own header comment
    gemini-adapter.js        gemini.google.com-specific DOM hooks — UNVERIFIED,
                            with an additional known Shadow DOM risk — see the
                            file's own header comment
    deepseek-adapter.js      chat.deepseek.com-specific DOM hooks — UNVERIFIED,
                            and the least confident of the four newer adapters
                            — see the file's own header comment
    copilot-adapter.js       copilot.microsoft.com (consumer web chat)-specific
                            DOM hooks — UNVERIFIED — see the file's own header
                            comment
  popup/
    popup.html / popup.js   Extension icon popup: THIS is where login happens —
                            an embedded login form shows automatically when not
                            logged in, connection status ("Connected as
                            you@example.com" with a green dot) when logged in,
                            session scan count, link to the live dashboard
  options/
    options.html / options.js   Secondary/advanced page — same login form,
                            plus the API base URL override (for local dev).
                            Reachable from the popup's "Advanced" link, not
                            required for normal use since the popup itself
                            handles login
  icons/                   Placeholder gold-shield icons (16/48/128px) — functional,
                            not designed, per this pass's explicit scope
```

The background service worker is the only place that talks to the network. Content
scripts message it via `chrome.runtime.sendMessage` and get a plain
`{ ok: true, data: {...} }` or `{ ok: false, error: "...", message: "..." }` response
back — this keeps every network/auth failure mode centralized in one file
(`background.js`) instead of duplicated across every future site adapter.

## Install and use (manual — see Verification below for why this is manual)

For the plain-language version of these same steps (no dev background assumed), see
[USER_GUIDE.md](USER_GUIDE.md). The steps below are the condensed developer version.

1. Open `chrome://extensions` (or `edge://extensions` on Edge).
2. Enable **Developer mode** (top-right toggle).
3. Click **Load unpacked**, and select this `extension/` directory.
4. Click the Lango icon in your toolbar. Since you're not logged in yet, the popup
   itself shows a login form directly — no menu-hunting required. Log in with a real
   Lango account (e.g. the seeded demo account — `compliance@lango.demo` /
   `LangoDemo123!`, see the main repo README) — the popup defaults to the live backend
   (`https://lango-backend-qwkx.onrender.com`). To test against a locally-run backend
   instead, click **Advanced: change API URL** at the bottom of the login form, which
   opens the full options page where you can set it to `http://localhost:8080` (see
   the main README's Setup section).
5. Once logged in, the popup switches to showing a green dot and **"Connected as
   compliance@lango.demo"** (or whichever account you used) — that's the confirmation
   login worked.
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
  that existed at the time (`cleared_no_entities`, `redacted_and_forwarded`,
  `blocked_low_confidence`), matching the same behavior verified elsewhere in this
  project via `curl`. The fourth decision, `redacted_low_confidence_review`, was added
  in a later pass (see `docs/SECURITY_PRIVACY.md`'s Human oversight row) and has not
  been independently re-run through this same `vm`-mock verification — `site-adapter.js`
  handles it identically to `redacted_and_forwarded` (see its own code comment), and
  the backend side of this decision is covered by `cargo test`, but the extension's own
  handling of this specific decision has only been reviewed, not exercised live.
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
- **The four newer adapters (`claude-adapter.js`, `gemini-adapter.js`,
  `deepseek-adapter.js`, `copilot-adapter.js`) carry every limitation above, plus
  they've never even had chatgpt.com's level of scrutiny — no live verification
  attempt was possible for any of the five, but chatgpt.com's selectors are at least
  based on a long-stable, widely-documented convention (`#prompt-textarea`). The other
  four are comparatively fresher guesses, with meaningfully different confidence
  levels between them — read each file's own header comment rather than assuming
  they're all equally likely to work. `gemini-adapter.js` in particular calls out a
  specific structural risk (a possible closed Shadow DOM around Gemini's composer)
  that could mean it doesn't work at all, not just "might break later" the way the
  others might.

## Local development

To test against a locally-run backend instead of the live Render one: run the backend
locally (see the main repo README's "Full stack" setup section), then in the
extension's options page set **API base URL** to `http://localhost:8080` and log in
with the seed script's demo credentials. `http://localhost:8080/*` is already declared
in `manifest.json`'s `host_permissions`, so no manifest edits are needed for this.

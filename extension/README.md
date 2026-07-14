# Lango Browser Extension (v0.1)

A Manifest V3 Chrome/Edge extension that puts Lango's redaction layer directly in
front of an AI chat site's web UI: before a prompt leaves your browser, it's scanned by
the real Lango backend (the same `/api/scan` endpoint documented in the main repo
README), and redacted or blocked according to the same fail-closed logic used
everywhere else in this project.

**New here? Read [USER_GUIDE.md](USER_GUIDE.md) instead** — a plain-language
install-and-use walkthrough for a first-time user (judge, mentor, tester), rather than
this file's developer-facing architecture/verification detail.

## Scope: five sites, two verified with a real loaded extension

This extension implements five sites, three of which ("response scanning + observability
+ hardening" task) also scan the AI's reply, not just the outgoing prompt:

| Site | Prompt scanning | Response scanning |
|---|---|---|
| chatgpt.com | **Verified working** — driven against a real, logged-in browser session (see Verification below) | Implemented, **not verified** — chatgpt.com itself remains unreachable for a full session (see below); response-turn selector is a moderate-confidence guess based on a widely-documented convention, not live-checked |
| gemini.google.com | **Verified working** — a full real extension, loaded and driven against a live, production, anonymous gemini.google.com session, including a real prompt→scan→send→reply→response-scan→banner round trip (see Verification below) | **Verified working**, same real session — composer and response-turn selectors both confirmed against live DOM, real streaming-timing data measured and used to set the debounce window |
| claude.ai | Implemented, **not yet verified** against a live page — still fully blocked by both methods tried (see below) | Implemented, **not verified**, and lower confidence than chatgpt.com's guess — no comparably well-established public convention for claude.ai's response markup to base it on |
| chat.deepseek.com | Implemented, **not verified** — confirmed *unreachable* from this dev environment by two independent methods (headless-browser navigation and a plain HTTP fetch both blocked, the latter by an active AWS WAF challenge) — see `content/deepseek-adapter.js`'s header comment | Out of scope — response scanning was added for chatgpt.com, claude.ai, and gemini.google.com only |
| copilot.microsoft.com (consumer web chat — not GitHub Copilot) | Implemented, extension itself not verified, but the composer selector is **confirmed against a real fetch of the live page's HTML** (`textarea#userInput`, `data-testid="composer-input"`) — see `content/copilot-adapter.js`'s header comment | Out of scope, same reason as chat.deepseek.com |

**gemini.google.com's verification is a genuine, unexpected step up from every other
site**, not an incremental improvement — see the Verification section below for exactly
what this means and what it doesn't.

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
     *structured* entity type). Nothing is sent. You get a banner explaining why in
     plain language (the backend's `user_message` — e.g. "This message may contain a
     bank account number we're not confident about"), NOT the backend's
     `reason_string`, which is full technical detail (entity type names, confidence
     scores, which detector matched) meant for the Audit Log, not this banner — see
     `backend/src/detection/plain_language.rs` and `ScanResponse`'s own doc comment
     for the split. You have to edit the prompt yourself and resubmit — the extension
     does not retry automatically.
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
5. **On chatgpt.com, claude.ai, and gemini.google.com** (product-depth task, "response
   scanning + observability + hardening"): once the AI's reply has finished streaming
   in and settles — detected by watching for a pause in DOM mutations, since there's
   no single event that means "the response is done" — `content/response-scanner.js`
   reads its final text and sends it to `POST /api/scan/response`, correlated to the
   audit_log row the originating prompt scan created. If it's flagged (a leaked
   secret, a sensitive entity that shouldn't appear in a reply, anything that looks
   unsafe), an amber warning banner appears — but **the AI's actual response is never
   modified, hidden, or altered**. See `detection::scan::scan_response`'s doc comment
   and `docs/ARCHITECTURE.md` for the full reasoning on why covertly changing content
   the user didn't write is a different, more concerning kind of intervention than
   redacting their own outgoing prompt. A clean response shows no banner at all —
   deliberately silent, so a banner only ever means something worth reading.

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
    response-scanner.js     Shared response-scanning orchestration (product-depth
                            task, Part 1) — MutationObserver-based debounce,
                            correlates a stabilised response back to its prompt
                            scan, calls POST /api/scan/response — loaded only for
                            chatgpt.com, claude.ai, gemini.google.com
    chatgpt-adapter.js       chatgpt.com-specific DOM hooks — prompt side
                            VERIFIED, response side NOT verified, see
                            Verification below
    claude-adapter.js        claude.ai-specific DOM hooks — UNVERIFIED, see
                            Known fragility and the file's own header comment
    gemini-adapter.js        gemini.google.com-specific DOM hooks — VERIFIED
                            against a real, live, anonymous session (both
                            composer and response-turn selectors) — see
                            Verification below
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

Read this section before trusting any specific site-adapter file in this extension.

### gemini.google.com — a real, loaded extension, driven end-to-end

**A previous version of this section stated that Playwright's Chromium "does not
support extensions at all" in this environment. That was wrong — not because
anything about the environment changed, but because the wrong Playwright API was
used to test it.** `chromium.launch()` does not reliably support loading unpacked
MV3 extensions; `chromium.launchPersistentContext(userDataDir, { args:
["--disable-extensions-except=<path>", "--load-extension=<path>", "--headless=new"]
})` does, and worked on the first attempt during the response-scanning task
("response scanning + observability + hardening"): a real background service
worker (`chrome-extension://.../background.js`) registered, and a real content
script logged `[Lango] content script active on gemini.google.com` in the page
console. See Questions.md item 26 for the full trail, including why the earlier
conclusion was reached in good faith with the tool available at the time.

**gemini.google.com also turned out to be reachable, and — unexpectedly — usable
without logging into a Google account at all.** `https://gemini.google.com/`
returns a real HTTP 200 (chatgpt.com and claude.ai remain blocked, see below) and
serves a working, anonymous "Ask Gemini" composer that accepts real prompts and
returns real model replies. This made a genuine, live, end-to-end test possible —
the first time any of this extension's five sites has had one:

1. Loaded the real, unpacked extension via `launchPersistentContext`, injected a
   fake JWT directly into the extension's own `chrome.storage.local` (via
   `serviceWorker.evaluate()`, bypassing only the login UI — not the interception
   logic), pointed `apiBaseUrl` at a tiny local mock server whose response
   *shapes* exactly mirror the real backend's (`models::ScanResponse` /
   `ScanResponseCheckResponse` — this tests the extension's own logic for real,
   not the detection engine, which has its own separate real backend test suite).
2. Typed a real prompt into the real, live Gemini composer and pressed Enter.
   Lango's real capture-phase interception fired, scanned it (via the mock),
   showed the correct "no sensitive entities — sending" banner, and then
   genuinely re-sent it — confirmed by a real query bubble appearing in Gemini's
   own UI with the exact text, proving the interception's
   `preventDefault`/`stopPropagation`/`stopImmediatePropagation` really did stop
   Gemini's own send handler on the first pass (the bubble did not appear until
   the deliberate resend).
3. Real Gemini produced a real reply. `content/response-scanner.js`'s
   `MutationObserver` + debounce + `findLatestResponseTurn` correctly detected it
   stabilise and called the mock `/api/scan/response` with the **correct
   correlated `audit_log_id`** (confirmed by the mock server's own request log)
   and the exact clean response text.
4. Mock set to return `flagged: true` → the real warning banner appeared,
   verbatim, in the real page (screenshotted). Mock reset to `flagged: false`,
   full sequence re-run → no banner at all, confirming the deliberate silence on
   a clean response.

**Composer and response selectors were separately confirmed against real, live
DOM** (not just "the flow completed," but the actual markup inspected directly):
`rich-textarea .ql-editor[contenteditable="true"]` for the composer (the Shadow
DOM risk this file used to warn about did not materialise — plain
`querySelector` sees through fine), `message-content` for the response text
(confirmed to hold clean text with no UI-chrome noise, and confirmed correct
chronological ordering across a real two-turn conversation). A real streaming
response was also measured directly: a 6-sentence reply took ~7.8 seconds to
fully stream, with individual pauses between mutation bursts as long as 2906ms
while still actively arriving — this measurement is what `DEBOUNCE_MS = 4000` in
`content/response-scanner.js` is actually based on, not a guess. Full numbers in
Questions.md item 26.

**What this gemini.google.com verification does NOT cover, stated plainly:** a
real, logged-in Google account session (only the anonymous path was tested — a
logged-in session's DOM could differ); `findSendButton` (Enter-key submission was
used throughout, never a button click); the `writeText` redaction path (the mock
always returned `cleared_no_entities`, so a redacted-and-resend was never
exercised in this specific test); and the real backend's detection engine in this
exact integration context (covered separately by `cargo test`, not by this
browser session).

### chatgpt.com — prompt side verified (earlier work), response side not

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

**NOT verified with a real loaded extension**: unlike gemini.google.com (above),
chatgpt.com itself remains unreachable from this environment — re-checked during the
response-scanning task, not assumed stale. A headless-browser navigation to
`https://chatgpt.com` still gets stopped by a Cloudflare bot-check interstitial
("Just a moment...") before the real app ever loads. A raw, unauthenticated HTTP
fetch (the same technique that worked for copilot.microsoft.com in an earlier pass)
DID succeed this time — HTTP 200, ~490KB of real app-shell HTML — and confirmed
`#prompt-textarea` is still the current, real composer id, which is genuinely new
information. But that page is the logged-out landing shell (it has a visible
"Sign in"/"Sign up" pair, not a conversation), so it could not confirm anything
about response-turn markup, and there is no OpenAI account available to get past
that shell regardless. (Loading the *extension itself* is no longer the blocker
here — see the gemini.google.com section above for the methodology correction;
`chatgpt.com` specifically remains unreachable as a full, authenticated session.)

**What this means practically**: the DOM-interception logic in
`content/chatgpt-adapter.js` — finding the composer, finding the send button, reading
and writing its text, and (new in this task) finding the latest response turn via
`[data-message-author-role="assistant"]` — is written from best-effort knowledge of
chatgpt.com's publicly documented UI patterns. The composer id is now confirmed
current; the response-turn selector is a moderate-confidence guess (a widely and
consistently documented convention across several years of public tooling, stronger
than most guesses in this project, but still genuinely unverified against a live
session). **You should verify this yourself** using the manual steps above before
relying on it. If it doesn't work, the file's own comments point at exactly which
selectors to check first.

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
- **The other adapters carry different confidence levels — read each file's own
  header comment rather than assuming they're all equally likely to work.**
  `gemini-adapter.js` is now the exception to most of this section: its composer and
  response selectors were confirmed against a real, live, loaded-extension session
  (see Verification above), including confirming that the closed-Shadow-DOM risk this
  file used to warn about does NOT apply — `document.querySelector` sees through
  fine. `claude-adapter.js` and `deepseek-adapter.js` remain the least confident —
  claude.ai is still fully blocked by every method tried (a raw fetch now redirects to
  `/login` and 403s even there, not just a silent block), and `deepseek-adapter.js` is
  confirmed genuinely unreachable (an active AWS WAF challenge, not merely "never
  tried"). `copilot-adapter.js`'s composer selector was confirmed against a real,
  direct fetch of copilot.microsoft.com's current HTML in an earlier pass. See
  Questions.md for the full investigation trail behind each of these.
- **Response scanning (the three sites it was added to) is a genuinely harder DOM
  problem than prompt interception, and is fragile in a different way** — stated
  plainly, per this task's own instruction to be honest about the added complexity.
  Prompt interception reacts to one well-defined user action (Enter/Send) on an
  element whose content is already final the instant that happens. A response
  streams in over several seconds with no equivalent "done" event; `content/
  response-scanner.js` approximates it with a debounce (see its own doc comment for
  the real, measured timing data behind `DEBOUNCE_MS`), which is a heuristic, not a
  guarantee — an unusually long pause mid-stream on a very long response could still
  cause a premature scan of truncated text, and this has only been measured against
  one site (gemini.google.com) out of the three it's used on. The prompt-to-response
  correlation mechanism (`LangoSiteAdapter.setLastScanId`/`getLastScanId`, a single
  mutable slot) is also a known, accepted simplification: it handles the common
  "one prompt, wait for the reply, then the next" pattern correctly, but a user
  sending a second prompt before the first response has stabilised can cause a
  response scan to attribute to the wrong audit_log row. See Questions.md item 26 for
  the full design reasoning and everything this specific verification pass did and
  did not confirm.

## Local development

To test against a locally-run backend instead of the live Render one: run the backend
locally (see the main repo README's "Full stack" setup section), then in the
extension's options page set **API base URL** to `http://localhost:8080` and log in
with the seed script's demo credentials. `http://localhost:8080/*` is already declared
in `manifest.json`'s `host_permissions`, so no manifest edits are needed for this.

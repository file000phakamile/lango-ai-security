# Lango Browser Extension — User Guide

A plain-language walkthrough for someone who has never seen this project before — a
judge, a mentor, a first-time tester. If you want the developer-facing technical
detail (architecture, file layout, exactly what was and wasn't tested), see
[extension/README.md](README.md) instead. This document is the "how do I actually use
this" version.

## 1. What this actually is, right now

Read this before doing anything else.

- **This is a v0.1 demo, not a finished commercial product.** It is real, working
  code — not a mockup — but it hasn't had a security-hardening pass, hasn't been
  tested at scale, and several of the sites it supports (see below) have never been
  confirmed against a real browser session.
- **It's a browser extension** (Chrome or Edge, Manifest V3). As of this pass, it
  supports five AI chat sites:
  - **chatgpt.com** — implemented **and verified working** in a real browser session.
  - **claude.ai**, **gemini.google.com**, **chat.deepseek.com**, and
    **copilot.microsoft.com** (Microsoft's consumer web chat — not GitHub Copilot,
    a separate, mostly IDE-embedded product this extension does not target) — all
    **implemented, but not yet verified** against a real, logged-in page. See
    [Caveats](#6-caveats) below before relying on any of these four.
- **There are two separate things in this project, for two different people —
  don't conflate them:**
  - **The extension** (what you're reading about right now) is what an **employee**
    installs in their own browser and uses day-to-day, while actually chatting with
    ChatGPT/Claude/etc. It intercepts prompts *before* they're sent.
  - **The dashboard** — the separate live web app at
    **https://lango-app-dusky.vercel.app** — is what a **compliance or IT officer**
    uses afterward, to review the audit log of what the extension (and the rest of
    the system) has been doing: what was redacted, what was blocked, what's flagged
    for review. An employee installing the extension does not need to open the
    dashboard, and a compliance officer reviewing the dashboard does not need the
    extension installed. They are two different tools for two different jobs.
- **There is no self-service signup.** This whole demo runs on **one shared account**:
  `compliance@lango.demo` / `LangoDemo123!` (this credential is already documented in
  the main repo README and is intentionally public — it only ever protects synthetic
  demo data, nothing real). A real multi-tenant product would give every employee
  their own account; this v0.1 does not do that yet — everyone using this demo logs
  in as the same one user.

## 2. Installing the extension

You do not need any prior experience with browser extensions or development tools for
this. Five steps:

1. Open a new tab and go to `chrome://extensions` (Chrome) or `edge://extensions`
   (Edge) — type this directly into the address bar.
2. In the top-right corner of that page, find the **Developer mode** toggle and turn
   it **on**. (This page normally hides "load your own extension" options unless this
   is on — it's not a special or risky mode, just the one that allows loading an
   extension that isn't from the Chrome/Edge Web Store.)
3. Three new buttons appear along the top: **Load unpacked**, **Pack extension**,
   **Update**. Click **Load unpacked**.
4. A file picker opens. Navigate to this repository and select the `extension/`
   folder itself (not a file inside it — the folder).
5. **What it looks like if it worked**: a new card appears on the extensions page
   titled "Lango — AI Data Guard", with a version number and a toggle switched on.
   **What it looks like if something's wrong**: instead of a new card, you'll see a
   red error box naming the problem (commonly "Manifest file is missing or
   unreadable" if the wrong folder was selected — go back to step 4 and make sure you
   selected `extension/` itself).

## 3. Finding and pinning the icon

This step trips people up on first use, and deserves its own callout rather than being
assumed obvious.

After loading the extension, its icon does **not** automatically appear in your main
toolbar next to the address bar. Instead, look for a small **puzzle-piece icon** near
the top-right of your browser window (to the right of the address bar, usually just
left of your profile picture). Click it — a dropdown list of every installed extension
appears, and "Lango — AI Data Guard" should be in that list.

Click the little **pin** icon next to Lango's name in that dropdown. This moves
Lango's own icon out of the puzzle-piece menu and directly into your visible toolbar,
so you don't have to open that dropdown every time. Once pinned, you should see a
small gold shield icon sitting directly in your toolbar — that's the one you'll click
for everything from here on.

## 4. Logging in

Click the Lango icon (now pinned in your toolbar). A small popup window opens, about
the width of a large phone screen.

**Since this is a fresh install, you are not logged in yet.** The popup will show, top
to bottom:

- A status line with a red dot and the text "Not logged in".
- A short line listing which sites the extension is active on (ChatGPT marked
  verified; the other four marked unverified).
- A red-tinted banner: "You're not logged in — log in below to enable prompt scanning
  on supported sites."
- An **Email** field and a **Password** field, right there in the popup — you do not
  need to hunt through a separate menu or settings page to find the login form.
- A gold **"Log in to Lango"** button.

Enter the demo credentials from section 1 above:
- Email: `compliance@lango.demo`
- Password: `LangoDemo123!`

Click **"Log in to Lango"**. For a moment the small message area below the password
field will read "Logging in…". If it succeeds, the popup immediately switches to a
**green dot** next to the text **"Connected as compliance@lango.demo"** — that's your
confirmation everything is working. Below that, you'll also see a running count of
"Prompts scanned this session" (starts at 0) and a button to open the live dashboard.

If login fails (e.g. a typo in the password), the message area turns red with a
specific reason instead ("Login failed (HTTP 401)" or similar) — the password field
clears itself so you can retype it.

## 5. Using it

Go to one of the supported sites (chatgpt.com is the one to try first, since it's the
one actually confirmed working). Type a prompt, and either press Enter or click that
site's own Send button — Lango intercepts that action before the site's own JavaScript
ever sees it.

The very first thing you'll see, every time, is a brief grey **"Lango: scanning
prompt…"** banner near the bottom of the page while the prompt is sent to Lango's
backend for a decision. What happens next depends on what was found:

**A clean prompt with no sensitive data** — e.g. "What's a good recipe for banana
bread?" You do **not** get silence: a brief **green** banner appears — "Lango: no
sensitive entities detected — sending" — and your prompt is sent on to the AI exactly
as you typed it. This banner disappears on its own after a few seconds; there is
nothing for you to do.

**A prompt that triggers redaction** — e.g. "My national ID is 63-123456A23, can you
help me draft a complaint letter?" Lango detects the national ID with high confidence.
You'll see a **gold** banner — "Lango: 1 entity redacted before sending" — and, briefly
visible in the composer itself before it's sent, your prompt's text visibly changes:
the ID number is replaced with a placeholder like `[REDACTED:NATIONAL_ID]`. The
*redacted* version is what actually gets sent to the AI, never your original text.

**A prompt that triggers the new low-confidence-name review path** — e.g. "Dear John
Moyo, please review the attached document." An ordinary-looking name is exactly the
kind of thing this path exists for: Lango's name-detection heuristic isn't confident
enough to treat this as a routine, clear-cut match, but it also isn't confident enough
to justify blocking you outright over what's very likely a false alarm. You'll see a
distinct **amber** banner — "Lango: redacted (low-confidence name match, flagged for
review)" — your prompt is still redacted and sent, exactly like the case above.
**The plain-language meaning of this banner: your message was sent, but a compliance
officer will see this specific event flagged separately in the audit log for them to
double-check later. You don't need to do anything — this is not a block, just a
heads-up that this one is going into a review queue.**

**A prompt that triggers a genuine block** — e.g. "Please refund via account
9988776655443 once approved." A bank account number is a *structured* entity type
(unlike a name), and Lango's design deliberately treats any low-confidence match on a
structured entity as too risky to guess about — so this **blocks** rather than
redacting-and-sending. You'll see a **red** banner in plain language, naming what kind
of information was involved without exposing internal detector names or confidence
scores — something like "Lango: blocked — This message may contain a bank account
number we're not confident about. Please review and remove or rephrase it before
sending." — and **nothing is sent anywhere**. (The full technical detail — exact
entity type, confidence score, which detector matched — is still recorded for a
compliance officer reviewing the Audit Log later; it's just not what shows up in this
banner.) This banner does not auto-dismiss; you have to edit your prompt yourself
(e.g. remove or rephrase the account number) and submit again. Lango never retries
this one automatically on your behalf.

**If the backend is unreachable** — you'll see a **red** banner along the lines of
"Lango: blocked — Failed to fetch. Prompt not sent." (the exact wording depends on
your browser's own network-error message). Same as a genuine block: nothing is sent,
and you have to try again yourself. **Read the Caveats section below before assuming
this means something is broken** — this specific failure mode is often just the
free-tier backend waking up from being idle, not a real problem.

## 6. Caveats

Stated plainly, not buried in fine print.

- **Render free-tier cold start.** The live backend this extension talks to runs on
  Render's free tier, which spins the server down after roughly 15 minutes of no
  traffic. If you haven't used Lango in the last 15 minutes or so, the *first* prompt
  you send afterward may take **30-60 seconds** to get a response — or, if your
  browser's own request times out before the backend finishes waking up, you may see
  the "Lango backend unreachable" fail-closed block described above. **If this
  happens: wait a few seconds and try again.** The backend is very likely just waking
  up, not actually broken. This is real, expected behavior of a free-tier host, not a
  bug in this extension.
- **DOM-based interception is inherently fragile, for every site, not just one.**
  This extension works by recognizing specific patterns in each site's own web page
  (a particular input box, a particular Send button). If any of these five sites
  changes its own web UI — even a routine redesign unrelated to AI safety at all —
  the extension can silently stop recognizing that site's input box or Send button
  until this project's code is updated to match the new layout. When this happens,
  the practical symptom is that the extension does *nothing at all* on that site (no
  banner, no interception) rather than failing loudly with an error — so if Lango
  suddenly seems to have stopped working on a site it used to work on, a UI change on
  that site's end is the most likely explanation, not a bug report waiting to happen.
  This is a standing limitation of this whole approach (client-side DOM
  interception), true of every site listed here, not a one-off issue with a single
  site.
- **Four of the five sites are implemented but genuinely unverified — read this
  before relying on them.** claude.ai, gemini.google.com, chat.deepseek.com, and
  copilot.microsoft.com were added using a best-effort, defensively written guess at
  each site's current input/Send-button structure. **None of the four has been loaded
  as a real extension and driven against a real, logged-in page** — this dev
  environment still has no display server, so Chromium never registers an extension
  service worker at all regardless of headless mode, re-confirmed directly (not just
  assumed) in a later pass. That said, "unverified via the extension" isn't the same
  as "no real information at all" for every site — a follow-up pass fetched
  copilot.microsoft.com's raw page HTML directly (no browser automation involved) and
  found the actual composer markup server-rendered in it: a plain `<textarea
  id="userInput" data-testid="composer-input">`, matching (and sharpening) what the
  adapter already guessed — see `content/copilot-adapter.js`'s header comment for the
  exact finding. The same technique against chat.deepseek.com was blocked outright by
  an active AWS WAF bot-verification challenge, confirming that site specifically
  really is unreachable from here by any available method, not just unverified by
  omission. Confidence varies noticeably by site — see each adapter file's own header
  comment (`content/claude-adapter.js`, `content/gemini-adapter.js`,
  `content/deepseek-adapter.js`, `content/copilot-adapter.js`) for the specific,
  honest reasoning behind each one, including one (Gemini) with a known structural
  risk — a possible closed Shadow DOM — that could mean it doesn't work *at all*, not
  just "might break later." **If you plan to actually rely on Claude, Gemini,
  DeepSeek, or Copilot support: test it yourself on a real page first**, the same way
  chatgpt.com's support still needs a first real manual test too — copilot.microsoft.com's
  composer selector is now better-founded than the others, but "better-founded" is
  still not "confirmed working end to end."
- **Only the five sites listed in section 1 are covered — nothing else.** This
  extension does not intercept anything happening in a desktop app, a mobile app, or
  an AI feature embedded *inside* another product — for example, GitHub Copilot
  inside VS Code, or Copilot features embedded inside Word/Excel/Outlook. It only
  ever sees what happens inside a browser tab open to one of the five domains listed
  above.
- **Single shared demo account, no real per-user attribution.** Every action taken
  through this extension right now is logged under the one seeded demo account
  (`compliance@lango.demo`) — there is no way, in this v0.1, to tell which real
  person actually typed a given prompt beyond what's already seeded. A real
  deployment would need real, distinct per-employee logins for that to mean anything.

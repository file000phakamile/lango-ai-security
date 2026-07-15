# How to Use Lango

A plain-language guide to both halves of this product, kept in sync with the
dashboard's own **Help** tab (open the dashboard and click Help in the sidebar for
the same content in the app itself). If you want developer-facing detail —
architecture, file layout, exactly what has and hasn't been tested — see
[extension/README.md](extension/README.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
instead. This page is the "how do I actually use this" version.

## The two parts of Lango

Lango is two separate tools for two different people — don't conflate them.

- **The browser extension** is what a frontline **employee** installs and uses
  day-to-day, while actually chatting with ChatGPT, Claude, Gemini, and similar
  tools. It scans a prompt *before* it's sent, and scans the AI's reply after it
  arrives.
- **The dashboard** — the web app at
  [lango-app-dusky.vercel.app](https://lango-app-dusky.vercel.app) — is what a
  **compliance or IT officer** uses afterward, to review what the extension has
  been doing: what was redacted, what was blocked, what's flagged for review.

An employee installing the extension does not need to open the dashboard. A
compliance officer reviewing the dashboard does not need the extension installed.

This is a v0.1 demo, not a finished commercial product — it's real, working code,
not a mockup, but it hasn't had a formal security audit and several of the sites
it supports have never been confirmed against a live browser session (see
Known Limitations below).

There is no self-service signup yet. This demo runs on one shared account:
`compliance@lango.demo` / `LangoDemo123!` — already public, and it only ever
protects synthetic demo data, nothing real.

## Using the extension

**Install it** (Chrome or Edge): go to `chrome://extensions`, turn on
**Developer mode** (top right), click **Load unpacked**, and select the
`extension/` folder. A new "Lango — AI Data Guard" card should appear.

**Find the icon**: it doesn't appear in your toolbar automatically. Click the
puzzle-piece icon near your address bar, find Lango in the list, and click its
pin icon to keep it visible.

**Log in**: click the Lango icon, enter the demo credentials above, and click
**Log in to Lango**. A green dot and "Connected as compliance@lango.demo" means
you're ready.

**Use it**: go to a supported site, type a prompt, and press Enter or click
Send as normal — Lango intercepts it first. A real scan is usually well under a
second; you'll typically go straight to the result banner below with no
"scanning" indicator in between.

## What each banner means

| Color | What happened | What you need to do |
|---|---|---|
| 🟢 Green | Nothing sensitive found — your prompt was sent unchanged. | Nothing. |
| 🟡 Gold | A sensitive entity was redacted before sending (e.g. a national ID). | Nothing — the redacted version was sent, not your original text. |
| 🟠 Amber | Either a low-confidence name match was redacted and sent (flagged for a compliance officer to review later), or the AI's *reply* may contain something sensitive. | Nothing for a redacted prompt. For a flagged reply: the AI's answer is shown to you in full and unchanged — review it yourself before relying on or sharing it. |
| 🔴 Red | Blocked — a structured sensitive value (bank account, national ID, etc.) was found but Lango wasn't confident enough to redact it safely, or the backend was unreachable. | Nothing was sent. Edit your prompt and try again, or wait a few seconds and retry if the backend was unreachable (see Cold Start below). |

Response scanning (checking the AI's *reply*, not just your prompt) currently
covers **chatgpt.com, claude.ai, and gemini.google.com** only, and takes longer
than prompt scanning — commonly under 10 seconds. A staged loading indicator
shows nothing for the first second, then a calm spinner, then a short status
message if it runs past a few seconds. The AI's reply itself is never changed,
hidden, or redacted by this check — only ever flagged with a banner next to it.

## Using the dashboard

The dashboard has no login screen of its own in this demo — it authenticates
automatically as the shared demo account. Each sidebar view:

- **Command Center** — a live overview: how many sessions were scanned today,
  how many were blocked or redacted, the average risk score, and any active
  monitoring alerts, plus a recent-events feed. Updates automatically every 15
  seconds when connected to a real backend.
- **Audit Log** — the full, filterable record of every scan: who, when, what was
  detected, the decision made, and why. Expand a row for full detail. A flagged
  low-confidence row can be confirmed or overturned by a reviewer directly here.
- **Fairness Audit** — compares how often prompts get flagged across different
  languages and departments, so a systematic bias in detection doesn't go
  unnoticed.
- **Drift & Security** — tracks whether detection accuracy is drifting over time,
  plus a feed of security-relevant events.
- **Pilot & Sandbox** — the current pilot's scope, rollout checklist, and
  success metrics.
- **Health Data Guard** — a version of the same monitoring scoped to
  health-related detections, deliberately reporting only totals and coarse
  splits (never a per-condition breakdown) to avoid indirectly identifying
  someone from a small aggregate.
- **Policy Builder** — lets a compliance admin adjust their organisation's
  detection sensitivity within safe bounds, and add organisation-specific
  detection patterns. Health-related detections always follow the strictest
  rule regardless of this setting — that one isn't configurable by anyone,
  deliberately.
- **Compliance Export** — one-click CSV/PDF export of the audit log, fairness
  metrics, and drift history for a date range, ready to hand to an auditor.
- **System Health** — a simple list of recent backend errors, so an operator can
  spot a problem without a separate monitoring tool.

## Known limitations that actually matter

- **Which sites are actually verified, not just implemented.** ChatGPT's prompt
  scanning and Gemini's prompt *and* response scanning have both been driven
  against real, live sessions and confirmed working. Claude, DeepSeek, and
  Copilot's consumer web chat are implemented using a best-effort guess at each
  site's structure but have not been confirmed against a live page — test them
  yourself before relying on them. (Full per-site detail, including exactly
  what was and wasn't tested, is in `extension/README.md`.)
- **The backend can take up to a minute to wake up.** The live backend runs on
  a free hosting tier that spins down after about 15 minutes of no traffic. The
  first request after an idle period can take 30-60 seconds, or may show a
  "backend unreachable" banner if your browser gives up first. This is normal —
  wait a few seconds and try again.
- **Mobile and small screens work.** The dashboard's sidebar collapses to a
  slide-out drawer and the audit log becomes a card list below 768px width —
  tested down to 375px with no horizontal overflow.
- **Response scanning is a genuinely harder problem than prompt scanning.** A
  reply streams in over several seconds with no single "it's done" signal, so
  Lango approximates "finished" by waiting for the page to stop changing. This
  is a measured, evidence-based approach, not a guess, but it's a heuristic —
  an unusually long pause mid-reply could in principle cause a very long
  response to be checked slightly early.
- **One shared demo account.** Every action in this demo is logged under one
  seeded user — there's no way yet to tell which real person sent a given
  prompt. A real deployment would give every employee their own login.

# How to Use Lango

A plain-language guide to both halves of this product, kept in sync with the
dashboard's own **Help** tab (open the dashboard and click Help in the sidebar for
the same content in the app itself). If you want developer-facing detail —
architecture, file layout, exactly what has and hasn't been tested — see
[extension/README.md](extension/README.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
instead. This page is the "how do I actually use this" version.

## The parts of Lango

Lango is a few separate tools for different people and different situations — don't
conflate them.

- **The browser extension** is what a frontline **employee** installs and uses
  day-to-day, while actually chatting with ChatGPT, Claude, Gemini, and similar
  tools. It scans a prompt *before* it's sent, and scans the AI's reply after it
  arrives. Works on any of the five sites it supports, and needs nothing provisioned
  by IT beyond the install itself.
- **The web app's native chat** (`/chat`) is a second way to get the same
  protection, built directly into this product rather than layered on top of a
  third-party site: type a message, it gets scanned and redacted the same way, and
  the (redacted) message goes to OpenAI using the organisation's own key — no
  extension install needed. This is the more complete, more robust path once an
  institution has actually rolled it out: every conversation gets the same durable,
  reviewable audit trail entry the extension's scans already get.
- **The dashboard** — the web app at
  [lango-app-dusky.vercel.app](https://lango-app-dusky.vercel.app) — is what a
  **compliance or IT officer** uses afterward, to review what's been happening
  (across both the extension and the native chat): what was redacted, what was
  blocked, what's flagged for review.

**Which one should you actually use?** If your institution has rolled out `/chat`
and provisioned an OpenAI key (see Policy Builder → Chat: OpenAI API Key), that's the
fuller, no-install path — use it. If you're on a site the web app doesn't cover
(ChatGPT, Claude, Gemini, etc. directly), or your institution hasn't set up `/chat`
yet, the extension is what covers you. Neither replaces the other — an employee using
the extension does not need `/chat`, and someone using `/chat` does not need the
extension installed. A compliance officer reviewing the dashboard does not need
either installed themselves.

This is a v0.1 demo, not a finished commercial product — it's real, working code,
not a mockup, but it hasn't had a formal security audit and several of the sites
it supports have never been confirmed against a live browser session (see
Known Limitations below).

There is no self-service signup yet. The dashboard's other views still run on one
shared account: `compliance@lango.demo` / `LangoDemo123!` — already public, and it
only ever protects synthetic demo data, nothing real. A real `/login` page now
exists specifically for `/chat` (and for seeing the dashboard as a genuinely
different role): try `staff1@lango.demo` / `LangoDemo123!` to see a staff login land
directly on `/chat` with no dashboard access at all, matching the real role model.

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

## Using the native chat (`/chat`)

No install needed — log in at `/login` and, if your organisation has provisioned an
OpenAI key (Policy Builder → Chat: OpenAI API Key, `compliance_admin` only), start
typing. The same scan runs on your message before anything is sent: a blocked
message shows the same red banner the extension shows and nothing is sent; a clean
or redacted message streams a reply back from OpenAI in real time. Every message you
send is scanned and, if needed, redacted before it ever reaches OpenAI — same rule as
the extension, just without needing a browser add-on.

If the AI's reply itself turns out to contain something sensitive, you'll see the
same amber warning described below, attached to that specific reply, usually within
a few seconds of it finishing — the reply itself is never changed or hidden, exactly
like the extension's own response scanning.

If you see "Your organisation has not configured an OpenAI API key yet," that's not
an error in your message — it means chat hasn't been set up yet; ask a compliance
admin to add one.

## What each banner means

| Color | What happened | What you need to do |
|---|---|---|
| 🟢 Green | Nothing sensitive found — your prompt was sent unchanged. | Nothing. |
| 🟡 Gold | A sensitive entity was redacted before sending (e.g. a national ID). | Nothing — the redacted version was sent, not your original text. |
| 🟠 Amber | Either a low-confidence name match was redacted and sent (flagged for a compliance officer to review later), or the AI's *reply* may contain something sensitive. | Nothing for a redacted prompt. For a flagged reply: the AI's answer is shown to you in full and unchanged — review it yourself before relying on or sharing it. |
| 🔴 Red | Blocked — a structured sensitive value (bank account, national ID, etc.) was found but Lango wasn't confident enough to redact it safely, or the connection was unreachable. | Nothing was sent. Edit your prompt and try again, or wait a few seconds and retry if the connection was unreachable (see Known Limitations below). |

Response scanning (checking the AI's *reply*, not just your prompt) currently
covers **chatgpt.com, claude.ai, and gemini.google.com** only, and takes longer
than prompt scanning — commonly under 10 seconds. A staged loading indicator
shows nothing for the first second, then a calm spinner, then a short status
message if it runs past a few seconds. The AI's reply itself is never changed,
hidden, or redacted by this check — only ever flagged with a banner next to it.

## Using the dashboard

The dashboard's other views still have no login screen of their own in this demo —
they authenticate automatically as the shared demo account, unless you've already
logged in via `/login` as a `compliance_admin`/`department_reviewer`. A "Chat" link
at the top of the sidebar goes to `/chat`. Each sidebar view:

- **Command Center** — a live overview: how many sessions were scanned today,
  how many were blocked or redacted, the average risk score, and any active
  monitoring alerts, plus a recent-events feed. Updates automatically every 15
  seconds when connected to the live system.
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
  deliberately. Also where a compliance admin provisions or rotates the
  organisation's OpenAI key for `/chat`, and sees a basic usage count — the key
  itself is never shown again once saved, only a masked confirmation.
- **Compliance Export** — one-click CSV/PDF export of the audit log, fairness
  metrics, and drift history for a date range, ready to hand to an auditor.
- **System Health** — a simple list of recent system errors, so an operator can
  spot a problem without a separate monitoring tool.

## Known limitations that actually matter

- **Which sites are actually verified, not just implemented.** ChatGPT's prompt
  scanning and Gemini's prompt *and* response scanning have both been driven
  against real, live sessions and confirmed working. Claude, DeepSeek, and
  Copilot's consumer web chat are implemented using a best-effort guess at each
  site's structure but have not been confirmed against a live page — test them
  yourself before relying on them. (Full per-site detail, including exactly
  what was and wasn't tested, is in `extension/README.md`.)
- **The system can take up to a minute to wake up.** The live system runs on
  a free hosting tier that spins down after about 15 minutes of no traffic. The
  first request after an idle period can take 30-60 seconds, or may show a
  "connection unreachable" banner if your browser gives up first. This is normal —
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
- **The native chat's connection to OpenAI has not been verified against the real
  API.** No live OpenAI key was available while building it — it was tested against
  a stand-in mock server, not the real one, except for one deliberate real test call
  with a fake key (to confirm errors are handled cleanly, which they were). Until
  someone with a real key tests it, treat the streaming/response behavior as tested
  but not confirmed against the real service.
- **One shared demo account.** Every action in this demo is logged under one
  seeded user — there's no way yet to tell which real person sent a given
  prompt. A real deployment would give every employee their own login.

# UI Copy Audit — developer-facing content leaking into end-user surfaces

A content-placement audit, distinct from `docs/WRITING_AUDIT.md` (which checked for
AI-tell phrasing). This pass checks for a different problem: text that is accurate
and well-written, but belongs in developer documentation, not in what an end user —
a compliance officer using the dashboard, or a staff member using the extension —
actually sees on screen. This is an inventory, not a rewrite — see the "What was
actually changed" section at the bottom for what was fixed and why, added after the
fixes were made.

**Scope covered**: every component under `components/lango/*.tsx`; the extension's
`popup/popup.html`, `popup/popup.js`, `options/options.html`, `options/options.js`;
every banner string in `extension/content/*.js`.

**Categories checked**: (1) file paths and source references shown directly in UI
text, (2) scope/versioning commentary written like a commit message rather than
something a user needs to know, (3) long justification paragraphs explaining *why*
a limitation exists when a user only needs to know *what* it is, (4) markdown syntax
appearing literally in rendered output instead of a real element, (5) long inline
caveat text that would be better as a short line plus a link to fuller
documentation.

## Method

Read every in-scope file in full (not just grepped), then ran targeted greps across
the same scope for `cargo run`, file-path patterns (`.rs`, `.md`, `backend/`,
`extension/`, `docs/`), `/api/` references, and literal markdown link/bold syntax
(`](`, `**`) to catch anything a full read might have skimmed past. The markdown-
syntax check was run project-wide (not just the in-scope files), since a rendering
bug of that kind isn't guaranteed to sit only in the files this task named.

## Findings

### Dashboard (`components/lango/*.tsx`)

| File | Location | Flagged text | Category | Why it needs to move or change |
|---|---|---|---|---|
| `system-health.tsx` | Panel `sub`, line 67 | "Recent backend errors (5xx responses only) - an internal fallback for third-party error tracking, since that needs an account only the deployment operator can provision" | 3 — long justification for a limitation | A compliance officer reading this panel needs to know *what* it shows, not *why* Sentry wasn't integrated. That reasoning is real and worth keeping, just not here — it's already recorded in Questions.md. |
| `system-health.tsx` | Footer paragraph, lines 122-126 | "Shows the 100 most recent 5xx responses across this deployment, not just this organisation — a known v1 scope limitation for a single/few-tenant pilot, stated explicitly in `backend/src/routes/backend_errors.rs`." | 1, 2 — file path + commit-message-style scope commentary | A literal source file path (`backend/src/routes/backend_errors.rs`) rendered as UI text, plus "v1 scope limitation" phrasing that reads like a code comment, not a sentence written for the person reading this screen. |
| `system-health.tsx` | Error state, line 81 | `Could not load backend errors: {loadError}` — surfaces the raw JS error (e.g. "Failed to fetch") | 3 — raw technical detail in place of a plain message | "Failed to fetch" is a browser networking error string, meaningless to a non-technical reader, and this task named this exact string as needing a plain-language rewrite. |
| `policy-builder.tsx`, `compliance-export.tsx`, `system-health.tsx` | Mock-mode fallback panels | "Start the backend (`cargo run`) and reload to use it." (three near-identical instances) | 1 — developer instruction shown to an end user | `cargo run` is a terminal command for someone with this repo checked out locally. A compliance officer using a hosted demo or a real pilot deployment has no terminal, no repo, and cannot act on this instruction at all. |
| `health-data-guard.tsx` | Empty-state message, lines 129-130 | "facility_type is an optional, caller-declared field (see docs/HEALTH_MODULE.md); this chart populates once at least one /api/scan call supplies it." | 1 — doc file path + API endpoint name | Same pattern: a doc reference and a raw API route name, neither actionable or meaningful to the person looking at an empty chart. |
| `policy-builder.tsx` | Threshold panel footer, line 194 | A single ~85-word sentence explaining the safe-bounds rationale and the special-category-health hard rule | 5 — long inline caveat where a short line + link would read better | Not developer-only content (a compliance officer does need to know this bound can't be widened here), but long enough to be worth trimming with the fuller reasoning available in the new help document. |

### Extension popup (`popup/popup.html`, `popup/popup.js`)

| File | Location | Flagged text | Category | Why it needs to move or change |
|---|---|---|---|---|
| `popup.html` | Footer note, lines 249-252 | `v0.1 — see <code>extension/USER_GUIDE.md</code> for a plain-language walkthrough, or <code>extension/README.md</code> for scope and known limitations.` | 1 — file paths, not clickable | This is the exact case the task named: a user reading the popup cannot open a file path from inside a browser extension popup. Also stale — the manifest is at v0.4.0, not v0.1. |
| `popup.html` | "Active on" line, lines 199-200 | `ChatGPT (verified)`, `Claude, Gemini, DeepSeek, Copilot (unverified)` | Factual — stale verification status | gemini.google.com has been verified end-to-end (prompt and response scanning) since the docs-accuracy pass — see Questions.md items 26/31/34. This line hasn't been updated to reflect that. |
| `popup.js` | `SUPPORTED_SITES`, line 11 | `{ host: "gemini.google.com", label: "Gemini", verified: false }` | Factual — stale internal tracking | The array this display line and the per-tab status line both read from still marks Gemini unverified. |
| `popup.js` | `refreshTabStatus()`, line 63 | `` `Active on this tab: ${match.label} (unverified adapter — see USER_GUIDE.md)` `` | 1 — file reference in dynamically-generated text | Same file-reference problem as the footer note, but constructed at runtime rather than static HTML — grep for the static string wouldn't have caught this one. |

### Extension options (`options/options.html`)

| File | Location | Flagged text | Category | Why it needs to move or change |
|---|---|---|---|---|
| `options.html` | Intro paragraph, lines 91-95 | "...see `extension/USER_GUIDE.md` for which of these are actually verified working versus implemented-but-unverified" | 1 — file path | Same unreachable-file-path problem as the popup. |
| `options.html` | Footer note, lines 113-119 | The entire note: "v0.1.", `chrome.storage.local` named directly, instructions to override the API URL to `http://localhost:8080` "to test against a locally-run backend", "see `backend/README` / the main repo README's Setup section for running the backend locally", "See `extension/README.md` for full install and testing instructions." | 1, 2 — file paths, JS API names, and local-dev developer instructions, all in one block | This entire paragraph is written for someone with the repo cloned and a terminal open, not for the actual end user of a browser extension options page. |

### Markdown-syntax rendering bug — investigated, not found in current source

The task described a literal `[Open live dashboard](url)` string rendering as plain
text instead of a real link. Searched exhaustively for this: grepped the full
project (every `.js`, `.ts`, `.tsx`, `.html` file, not just the files this task
named) for markdown link syntax (`](http`), markdown bold syntax (`**word`) outside
of `.md` files and `//`/`/* */` code comments, and the literal phrase "open live
dashboard" case-insensitively. **Not found.** `popup.html`'s "Open live dashboard"
link (line 243-245) is already a real `<a class="button primary" href="https://
lango-app-dusky.vercel.app" target="_blank" rel="noopener">` element — a genuine,
working, correctly-styled button-link, not literal bracket-paren text. No other
instance of literal markdown syntax in rendered output was found anywhere in
`components/`, `lib/`, or `extension/*.js`/`*.html`. This is recorded here rather
than silently skipped, and re-confirmed visually in a real browser during
Verification (see below) rather than only by grep.

### Extension banner strings (`content/*.js`)

Read every banner string in `site-adapter.js`, `response-scanner.js`, and
`ui-banner.js` (already substantially rewritten in the immediately preceding
performance/design pass). No file paths, source references, or developer-facing
scope commentary found in any rendered banner or indicator string — the "unverified
adapter — see USER_GUIDE.md" string in `popup.js` above is the only file-reference
instance found anywhere in extension runtime strings.

### `extension/USER_GUIDE.md` (not itself in scope, but affected by the fix)

Line 100 describes the popup's own "Active on" line ("ChatGPT marked verified; the
other four marked unverified") — this becomes stale the moment the popup is fixed,
so it's updated alongside it even though `USER_GUIDE.md` itself wasn't part of this
task's audit scope.

## Judgment call — the "Claude now verified" premise

This task's Part 3 states "ChatGPT, Claude, and Gemini are now confirmed verified."
Before writing that claim into any file, re-tested claude.ai directly, live, in this
session: a raw HTTP fetch returns `403`, and a real headless-browser navigation
also returns `403` with a Cloudflare "Just a moment..." interstitial — identical to
every previous attempt documented in this project (Questions.md, `extension/README.md`,
`content/claude-adapter.js`'s own header). **Claude is not marked verified anywhere
in this pass, despite the instruction, because doing so would be a false claim this
session's own fresh evidence directly contradicts.** Gemini's verification is real
and independently well-established (Questions.md items 26/31/34) and is updated
everywhere it was still stale. ChatGPT's prompt-side verification was already
accurately marked before this task and required no change. Full reasoning in
Questions.md; flagged prominently to the user in this session's own summary as well,
since a direct instruction was not followed as literally stated.

## What was actually changed

See the per-part commits and their own descriptions for the full list; summarized
here for the audit record, added after the fixes below were made:

- `system-health.tsx`: panel `sub` shortened to a plain description; the file-path/
  scope-limitation footer paragraph removed (reasoning moved to `HOW_TO_USE.md` and
  a code comment in `backend_errors.rs`); the raw fetch-error message replaced with
  a plain-language equivalent.
- `policy-builder.tsx`, `compliance-export.tsx`, `system-health.tsx`: the `cargo
  run` mock-mode message replaced with a message that doesn't assume terminal
  access.
- `policy-builder.tsx`: the long threshold-bound paragraph shortened, with the full
  reasoning available in `HOW_TO_USE.md`.
- `health-data-guard.tsx`: the doc/API-reference empty-state message replaced with
  plain language.
- `popup.html`: the footer note replaced with one line and a real link to the new
  help surface; the "Active on" line updated to reflect Gemini's real verified
  status.
- `popup.js`: `SUPPORTED_SITES`'s `gemini.google.com` entry set to `verified: true`;
  the per-tab status string's file reference removed.
- `options.html`: both the intro paragraph's file reference and the entire
  developer-facing footer note replaced with a short, plain equivalent and a link
  to `HOW_TO_USE.md`.
- `policy-builder.tsx`, `compliance-export.tsx`: the same `cargo run` mock-mode
  instruction found in `system-health.tsx` was also present here — fixed
  consistently across all three.
- `policy-builder.tsx`: the ~85-word threshold-bound paragraph shortened to one
  sentence pointing at the new Help tab; the reasoning it points to was added to
  `HOW_TO_USE.md`/`help.tsx` so the pointer resolves to something real.
- `health-data-guard.tsx`: the `docs/HEALTH_MODULE.md` and `/api/scan`
  references removed from the empty-state message.
- `pilot-status.tsx`, `drift-monitor.tsx`: re-checked against the same
  checklist, no findings — left unchanged.

## Verification (after all fixes)

**Dashboard Help tab**: confirmed live in a real browser (`http://localhost:3000`,
Playwright, headless Chromium) — the Help nav item is visible in the sidebar,
clicking it renders the real Help panel content, and navigating directly to
`http://localhost:3000/#help` on a fresh page load also lands on the Help view
with the header correctly reading "Help" — the hash-based deep link the popup's
help routing depends on genuinely works, not just in theory. Screenshotted
(`help-tab-via-sidebar.png`, scratch-only).

**Extension popup links**: confirmed live, DOM-inspected, in a real loaded
extension (`chromium.launchPersistentContext` + `--load-extension`, the method
established in Questions.md items 26/31/34), across three states:

- **Logged out** (no stored role yet): the help link's `href` resolves to the
  public GitHub-hosted `HOW_TO_USE.md` — the safe default before a role is known.
- **Logged in as `compliance_admin`**: the help link's `href` switches to
  `https://lango-app-dusky.vercel.app/#help` — the dashboard's own Help tab,
  correctly deep-linked.
- **Logged in as `staff`**: the help link stays on the public GitHub URL, exactly
  as designed — a staff user is never pointed at dashboard access they don't
  have.

**The "Open live dashboard" link specifically**: DOM-inspected directly —
`tagName: "A"`, a real `href`, and (while visible in the logged-in state)
`getComputedStyle` confirms `cursor: pointer` and `textDecoration: none` (styled
as a button, not literal underlined bracket text). Screenshotted
(`popup-logged-in.png`, scratch-only) — visually a real gold button, not
bracket-paren text. This is the definitive confirmation of the item 37/39
finding: there was nothing to fix here, and this is now proven by direct
inspection of the rendered DOM, not just a grep result.

**Full test suite**: `cargo test --lib` — 116 passed, 0 failed. `cargo test
--no-run` — all 8 integration test files, including
`multi_tenant_isolation.rs`, compile cleanly. `npm run build` — clean throughout
every part of this pass. No backend Rust code was touched by this task at all,
so this result was expected, not a surprise, and was still run and checked
rather than assumed.

---

# Pass 2 — "backend"/"mock data"/dev-command wording in fallback states

A follow-up, narrower pass: not about content placement this time, but about
specific word choice in the dashboard's two fallback states (a mock-data view
when the live connection isn't reachable, and a no-fallback empty state for the
three views where faking a number would be actively misleading). The goal is
accuracy and tone, not concealment — the distinction between real and sample
data stays completely visible; only the wording describing *why* something
isn't live changes.

**Scope covered**: every file under `components/lango/`, plus `HOW_TO_USE.md`
(not itself under `components/lango/`, but kept hand-synced with `help.tsx`
since the prior pass — see that file's own header comment — so a wording
change in one without the other would immediately go stale).

## Method

`grep -i` across `components/lango/` for `mock data`, `backend unavailable`,
`live backend`, `cargo run`, and the bare word `backend`, then read every hit in
its real surrounding context to separate rendered UI text from code comments,
import statements, and TypeScript type/function names (which aren't user-facing
and were left alone). Cross-checked `HOW_TO_USE.md` for the same terms since it
mirrors `help.tsx`.

## Findings — every instance of rendered text using "backend," "mock data," or
a developer command

| File | Line(s) | Flagged text | Category |
|---|---|---|---|
| `lango-dashboard.tsx` | 220 | `"mock data (backend unavailable)"` (the header status badge) | 1 — the main mock-data-fallback badge |
| `system-health.tsx` | 55 | "System Health needs a live backend connection — there's nothing real to show from mock data." | 2 — no-fallback empty state (the exact example this task quoted) |
| `system-health.tsx` | 51 | Panel `sub`, mock branch: "Recent backend errors and uptime status" | word choice |
| `system-health.tsx` | 66 | Panel `sub`, live branch: "Recent backend errors, so a problem can be spotted before a user reports it" | word choice |
| `system-health.tsx` | 80 | "Could not load recent backend errors right now. Try refreshing in a moment." (a live-branch fetch failure, not the no-connection case, but the same "we couldn't load this" shape) | word choice + consistency |
| `system-health.tsx` | 89 | "No backend errors recorded. Everything looks healthy." (the real, live, zero-errors success state) | word choice |
| `policy-builder.tsx` | 62 | "Policy Builder needs a live backend connection — there's no mock-data version of this view..." | 2 — no-fallback empty state |
| `compliance-export.tsx` | 39-40 | "Compliance Export needs a live backend connection — there's nothing real to export from mock data." | 2 — no-fallback empty state |
| `audit-log.tsx` | 59 | "Confirming/overturning this flagged row requires the live backend." (inside `ReviewSection`, shown for a flagged row when viewing mock data) | word choice |
| `help.tsx` | 90 | "...or wait a few seconds if the backend was simply unreachable." (describing the extension's red banner) | word choice |
| `help.tsx` | 112 | "A simple list of recent backend errors..." (describing the System Health view) | word choice |
| `help.tsx` | 133 | "The backend can take up to a minute to wake up." | word choice |
| `HOW_TO_USE.md` | 61 | Mirrors `help.tsx:90` | word choice |
| `HOW_TO_USE.md` | 78 | "...seconds when connected to a real backend." | word choice |
| `HOW_TO_USE.md` | 100 | Mirrors `help.tsx:112` | word choice |
| `HOW_TO_USE.md` | 112, 115 | Mirrors `help.tsx:133`, plus a second "backend unreachable" mention | word choice |

**Not flagged, checked and confirmed non-rendered**: `system-health.tsx`'s
`fetchBackendErrors` (an imported function name) and `BackendErrorEntry` (an
imported TypeScript type name) — both identifiers, never displayed as text —
and every `//`/`///` code comment across all files above that happens to
contain the word "backend" while describing implementation, not UI copy (e.g.
`policy-builder.tsx`'s doc comment referencing `backend/src/detection/scan.rs`,
already correctly left as a code comment in the prior pass).

**Not in scope, deliberately**: the extension's own banner strings
(`extension/content/*.js`, e.g. `site-adapter.js`'s "Lango backend unreachable"
fail-closed message) and `extension/popup/popup.html`. This task's own context
section frames the problem specifically as "this dashboard['s]... two different
fallback behaviors" — the extension's banners are a different system serving a
different moment (a live scan attempt failing, not a dashboard view falling
back to sample data), and weren't named. Flagged here rather than silently
included or silently skipped, in case a future pass wants the same treatment
applied there too.

## What was actually changed (Pass 2)

- **`atoms.tsx`**: added `UnavailableNotice`, a shared, zero-prop component for
  the "no live connection, probably temporary" state — "We couldn't load this
  just now. If nothing has been used recently, the system may still be starting
  up — this can take up to a minute. Please try again shortly." Never mentions
  mock/sample data, since none of the three views that use it ever show any.
  Extended `Badge` with an optional `title` prop (a native tooltip) for the
  header badge's "up to a minute" detail — see Questions.md item 43 for why a
  tooltip was chosen over a second visible line.
- **`lango-dashboard.tsx`**: the header status badge's mock-mode text changed
  from `"mock data (backend unavailable)"` to `"sample data — connecting to the
  live system"`, with a new hover tooltip ("This can take up to a minute after
  a period of inactivity."). Color unchanged — already amber (`#8A6323`), never
  red, both before and after.
- **`system-health.tsx`**: the no-connection empty state, the live-branch fetch-
  failure message, and the Panel `sub` text (both branches) now use
  `UnavailableNotice` or drop the word "backend"; the zero-errors success
  message reworded from "No backend errors recorded" to "No errors recorded."
- **`policy-builder.tsx`**, **`compliance-export.tsx`**: their no-connection
  empty states now use the shared `UnavailableNotice` component instead of a
  bespoke, separately-worded message each.
- **`audit-log.tsx`**: `ReviewSection`'s "Confirming/overturning this flagged
  row requires the live backend." reworded to "Confirming or overturning this
  needs a live connection — try again once reconnected."
- **`help.tsx`** and **`HOW_TO_USE.md`** (kept in sync): every remaining use of
  "backend" reworded to "connection" or "system" — the red-banner explanation,
  the System Health description, the cold-start explanation, and the Command
  Center description's "connected to a real backend" → "connected to the live
  system." One incidental fix alongside: `HOW_TO_USE.md`'s reference to a
  non-existent "Cold Start" section corrected to the section's real name,
  "Known Limitations."

## Verification (Pass 2)

Live browser (Playwright, `localhost:3000` against the real production
backend — CORS naturally rejects that from this origin, genuinely exercising
both fallback states, not simulating them): the header badge's new text and
tooltip render correctly and stay amber, confirmed via `getComputedStyle`
(`rgb(138, 99, 35)`, never red); System Health, Policy Builder, and Compliance
Export each render the new shared message and were checked programmatically to
contain the word "backend" nowhere in their rendered text; no horizontal
overflow at 375px width despite the badge's new text being longer than the old
one. `cargo test --lib`: 116/116, unchanged (no backend Rust code touched).
`cargo test --no-run`: all 8 integration test files compile. `npm run build`:
clean.

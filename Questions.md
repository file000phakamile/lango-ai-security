# Questions / assumptions

Nothing here blocked completion. Logging a few judgment calls for visibility.

## 1. Route structure
Used `app/page.tsx` as the dashboard's home route rather than `app/dashboard/page.tsx`.
Assumption: since this is a single-view demo (the sidebar switches panels client-side,
it doesn't navigate between routes), putting it at `/` is simpler and matches how a
judge/reviewer would expect to open the app. Easy to move under `/dashboard` later if
you want `/` free for a landing page.

## 2. Favicon
The task said not to spend time designing one, but a sensible default was easy: added
`app/icon.svg`, a small shield in the sidebar's gold (#8A6323) on dark background,
picked up automatically by Next's App Router icon convention. Original `favicon.ico`
from the scaffold is left in place as a fallback.

## 3. Mock-data PRNG (behavior fix, not a literal 1:1 port)
The source artifact's `generateAuditLog` and the `DRIFT_WEEKS` array shared one
module-level `mulberry32(2026)` instance. That's fine in the Claude.ai sandbox (pure
client-side, module re-evaluates fresh on every page load), but breaks in Next.js:
the dashboard is a "use client" component rendered once on the server (SSR) and then
again on the client during hydration. With a shared PRNG, those two runs consume the
random sequence in different orders and produce **different** audit-log rows, which
caused a React hydration mismatch and non-reproducible data on every refresh.

Fix: `generateAuditLog()` and `DRIFT_WEEKS` each now construct their own
`mulberry32(2026)` instance (`lib/lango/mock-data.ts`). Same seed, same formulas, same
distributions — this doesn't touch any of the hardcoded numbers from the proposal
(DIR/SPD worked example, week-9 PSI/KL spike, pilot metrics are all untouched literal
constants). It only makes the procedurally-generated audit log rows stable across
server/client renders, which is what the original code's own comment
("Deterministic PRNG so the mock data is stable across renders") already promised.

## 4. Visual verification caveat
Automated screenshots taken in this sandbox rendered the whole page color-inverted
(dark background instead of the intended light palette). I isolated this to a
Chrome/OS-level forced-dark accessibility feature in this environment — confirmed by
screenshotting a plain hardcoded white test page, which inverted identically. Added
`color-scheme: light` to `globals.css` as a correct defensive fix regardless.
**Update (copy-fix pass):** a later screenshot in this same sandbox rendered correctly
in true light mode (white panels, gold/red/green status colors, dark text) — so the
earlier inversion was an intermittent sandbox/OS artifact, not anything wrong with the
app. Light-mode rendering is now directly confirmed, not just inferred from code review.

## 5. Copy-fix round: source file on disk was unchanged
You said you'd updated `lango-dashboard.jsx` to remove "the submitted proposal" /
"Section X.X" references, and asked me to diff the new file against what I ported last
time. When I re-read `lango-dashboard.jsx`, its size, modification timestamp, and full
contents were byte-identical to the version from the original port — the five
proposal/Section references were still present verbatim. Your edit doesn't appear to
have been saved to that file.

Rather than block on this, I applied the fix directly based on your explicit
instructions (remove references to "the submitted proposal" / "Section X.X"; make the
copy read as a standalone product, not a companion to a document). Changed, in the
ported Next.js source only (`lango-dashboard.jsx` in this directory was left untouched
since I don't want to silently overwrite a file you may still be editing):

- `components/lango/fairness-audit.tsx`: "Quarterly Language Parity Check" subtitle →
  "Quarterly comparison of flag rates by session language, recalculated against live
  audit log data". DIR-fail alert's trailing clause "Mandatory pattern-rule review
  opened, per Section 2.2 of the proposal." → "Mandatory pattern-rule review opened
  automatically."
- `components/lango/pilot-status.tsx`: "Pilot Scope" subtitle → "Candidate institution
  status for the current pilot". "Midpoint Success Metrics" subtitle → "Agreed with the
  pilot institution before launch" (dropped the trailing "per Section 3.3").
- `lib/lango/mock-data.ts`: also tidied a source comment carrying the same phrasing
  (not user-facing, but same issue) — "Fairness data - language parity snapshot" with
  the Section 2.2 clause dropped.

Confirmed via a scripted pass over all five views that no "proposal" or "Section \d"
text remains anywhere in the rendered UI. If you did intend specific alternate wording
(rather than my rewrite), the four spots above are the ones to compare against your
original edit.

## 6. Deployment: Git wasn't installed on this machine
`git` was not found on PATH. Installed it via `winget install --id Git.Git -e` (silent,
standard package, low risk) since you explicitly asked me to git init/commit. No global
git identity existed either, so I set `user.email`/`user.name` scoped to this repo only
(not `--global`) using your known email, with the name "Lango Dev" — a placeholder,
change it if you want commit authorship to say something else:
`git config user.name "Your Name"` in `lango-app/`.

## 7. Deployment: production URL vs. GitHub-linked deploys
Went with direct CLI deploy (`vercel --prod --yes`) as you specified, not a GitHub
connection. This means future changes need `vercel --prod` run manually (or a git
integration set up later via the Vercel dashboard) — there's no auto-deploy-on-push.
Didn't set a custom domain or vanity project name beyond what the CLI auto-derived from
the directory name (`lango-app`); the alias came out as `lango-app-dusky.vercel.app`.

**Live URL: https://lango-app-dusky.vercel.app**

## 8. AI4I submission documents — judgment calls and open items

Produced the full AI4I submission document set (README, LICENSE, .env.example,
docs/BUSINESS_MODEL.md, docs/DEPLOYMENT_PLAN.md, docs/ARCHITECTURE.md,
docs/architecture-diagram.svg, docs/DATA_AI_USAGE.md, docs/UX_DESIGN.md,
docs/SECURITY_PRIVACY.md, docs/TESTING_LOG.md, docs/TEAM_CAPABILITY.md). A few items
were genuinely unknown and left as explicit placeholders/TODOs rather than invented:

- **No proposal PDF exists in this repo or its parent directory.** Searched both
  (`**/*.pdf`) and found none — only `lango-dashboard.jsx` (the pre-port source
  artifact) one level up. All document content is therefore drawn from the actual
  code in this repo (`lib/lango/types.ts`, `lib/lango/mock-data.ts`, the five
  dashboard components) plus the context given directly in the task prompt, not from
  a proposal document. If a proposal PDF exists elsewhere and should be the source of
  truth for figures/wording, it wasn't available to reconcile against here.
- **Live deployed URL**: reused `https://lango-app-dusky.vercel.app`, already recorded
  above from the prior deployment session (item 7) and confirmed still referenced
  consistently — did not re-verify it's still live via a fresh request in this pass.
- **Vanessa Moyo's background, role, and skills are unknown** and were left as
  explicit `_TODO_` placeholders in `docs/TEAM_CAPABILITY.md` rather than invented.
  Team should fill in: role, skills/background, and contribution area.
- **No real user/pilot feedback exists yet.** Stated plainly as "None yet" in
  `docs/UX_DESIGN.md` rather than fabricated.
- **Testing**: ran `npm run dev`, drove a headless Edge browser (via Playwright,
  installed ad hoc into the scratchpad since no browser-testing tooling exists in
  this repo) through all five dashboard views plus the Audit Log row-expand and
  decision filter. All rendered correctly, no console errors, figures internally
  consistent. One genuine, minor, dev-only cosmetic finding logged in
  `docs/TESTING_LOG.md`: Next.js's own dev-mode indicator badge overlaps the
  sidebar's footer disclaimer text locally (not present in the production/Vercel
  build) — no code fix applied since it's not part of this app's own code.
- **`docs/TESTING_LOG.md` intentionally has only one real row** plus a TODO marker,
  per the task instruction not to fabricate additional entries — the team should add
  further rows as they do their own manual testing, especially on mobile viewports
  and with accessibility tooling, neither of which has been tested at all.

## 9. Thorough testing pass + video/pitch content — judgment calls and open items

- **Real, significant mobile bug found and deliberately NOT fixed.** A thorough pass
  at 375px width (headless Edge via Playwright, same ad hoc setup as before) found the
  sidebar's fixed 224px width has no responsive breakpoint, squeezing all content into
  ~150px on every view — KPI values and chart labels cut off, Audit Log reduced to one
  column. This did not show up in a naive `document.scrollWidth` overflow check (no
  page-level overflow occurs — content is clipped/wrapped, not overflowing), only in
  visual screenshot inspection. Root cause is identified (`components/lango/
  lango-dashboard.tsx`'s `w-56 shrink-0` sidebar, no `sm:`/`md:` responsive variant)
  and documented in detail in `docs/TESTING_LOG.md`, but a real fix (collapsible
  sidebar behind a toggle, responsive grid breakpoints) is a genuine UI change with
  its own regression risk on desktop — explicitly left as an open item per this
  task's own instruction not to rush a bigger fix. Updated the README and
  `docs/UX_DESIGN.md` known-limitations wording from "not formally verified" to
  "verified broken" so those docs stay honest now that this has actually been tested.
- **Everything else tested passed cleanly**: sidebar navigation, the Command Center's
  animated request-trace (sampled programmatically across a full ~9.5s cycle plus a
  poll for the completed-state badge), the Audit Log's filter dropdown (all 4 options)
  and row expand/collapse (including the single-expanded-row behavior), chart hover
  tooltips on both the Fairness Audit and Drift & Security views, and a 4x refresh
  consistency check confirming the prior session's PRNG/hydration fix (item 3) still
  holds with zero hydration warnings. All logged in `docs/TESTING_LOG.md`, including
  the passes, not just the one failure, per this task's instruction.
- **Video script duration**: timed by word count only (~200 words ≈ 80s at 150wpm),
  per the task's own stated method — did not attempt to time actual screen-recording
  pauses (clicks, the ~7-8s trace animation), which would add real seconds beyond the
  spoken-word estimate. Flagged this explicitly in `docs/VIDEO_SCRIPT.md` itself so
  whoever records it knows to watch the total length, not just read the word count.
- **Pitch deck content** pulls figures directly from `docs/BUSINESS_MODEL.md` and
  `docs/DEPLOYMENT_PLAN.md` rather than restating them loosely, to avoid the two
  documents drifting apart over time.

## 10. Vanessa Moyo's role — partially filled, still needs team confirmation

Filled in `docs/TEAM_CAPABILITY.md` with **Role: Researcher**, as instructed directly.
Searched the whole repo (proposal, all commits via `git log --all -p`, this file) for
any existing mention of her specific research domain, institution, or focus area —
found none. Her specific focus is therefore left as an explicit placeholder
(`[Vanessa's research focus — to be confirmed by team]`), not guessed.

Added three *possible* contribution areas as clearly-labelled inference, not fact —
regulatory/compliance research, market/user research, and fairness/bias research —
since all three are genuinely plausible fits for a "Researcher" on this specific
product, but none is confirmed anywhere. **Team: please confirm Vanessa's actual
research focus and contribution area and replace the placeholder + inferred list in
`docs/TEAM_CAPABILITY.md` before final submission** — as instructed, this is flagged
here rather than settled unilaterally. Her "Skills / background" and "Contribution
area" fields remain `_TODO_`, unchanged, since only the role was specified this round.

## 11. Real backend build — database choice, environment blockers, bugs found by actually running it

This is the session that took Lango from a frontend-only mock demo to a real,
end-to-end-verified system: Rust + Axum API, PostgreSQL schema, a working
detection engine, seed script, and frontend wiring. Judgment calls and genuine
blockers below.

**sqlx over Diesel — no strong reason to deviate.** sqlx's async-native design fits
Axum's async runtime directly (Diesel's core is sync; async support is a bolted-on
wrapper), and its runtime-checked queries (`query_as::<_, T>`) don't require a
`DATABASE_URL` at *compile* time the way Diesel's schema macros or sqlx's own
`query!`/`query_as!` compile-time-checked macros do — simpler for a v0.1 that
needs to build in CI or on a machine without a live DB. Went with sqlx as the
task allowed.

**Frontend has no login screen — a demo-only shortcut, not a target-architecture
decision.** The dashboard authenticates as one fixed seeded `compliance` account
automatically (`lib/lango/api-client.ts`), since building a real multi-user login
UI wasn't in scope for wiring up the five *existing* dashboard views. The demo
account's password is intentionally committed in the open (`.env.local.example`,
`backend/src/bin/seed.rs`, README) since it only ever protects synthetic local
data — flagged here explicitly as a v0.1 shortcut, not something to carry into a
real deployment.

**Environment had no Rust linker, no Docker, and an inaccessible existing
Postgres — worked around all three rather than stopping:**
- No MSVC Build Tools and no MinGW/gcc were present, so `cargo check` failed at
  the link step on the very first attempt. Asked you directly rather than
  guessing; you chose the lightweight-GNU-toolchain option. Installed
  `BrechtSanders.WinLibs.POSIX.UCRT` (MinGW-w64 GCC) via `winget` and switched
  the default Rust toolchain to `stable-x86_64-pc-windows-gnu` via `rustup`.
  Both are standard, reversible dev-tool installs (`rustup toolchain uninstall`
  / uninstall the winget package to revert) — not touched anything else.
- Docker Desktop is not installed, so `docker-compose.yml` (still the
  documented, recommended path in README for anyone who *does* have Docker)
  couldn't be exercised directly. Two PostgreSQL server installs (17 and 18)
  were already present and running natively as Windows services on port 5432,
  but their superuser password was unknown to me and I did not attempt to
  guess or reset it — that's your credential, not mine to touch. Instead, used
  the same already-installed `initdb`/`pg_ctl` binaries to stand up a second,
  fully separate PostgreSQL data directory on port 5433 with fresh
  known credentials (`lango`/`lango_dev_password`, matching
  `docker-compose.yml`'s values), scoped to a scratch directory outside the
  repo. This is what actually ran migrations, the seed script, and every
  verification query in this session — your existing Postgres installs and
  their data were never touched. That scratch instance is not something this
  repo depends on going forward; `docker-compose.yml` (or your own Postgres on
  its usual port 5432) is the real, intended local setup.

**Three real bugs caught only by actually compiling and running the code — not
by review, which is exactly why "run it yourself" was worth doing:**
1. `backend/Cargo.toml`'s `sqlx` dependency listed `default-features = false`
   without including sqlx's `"macros"` feature, so every `#[derive(sqlx::FromRow)]`
   failed to compile. Added the feature.
2. `detection::rules::API_KEY_GENERIC_RE` used regex lookahead
   (`(?=...)`) to require "at least one digit AND one letter" in a long token —
   Rust's `regex` crate does not support look-around at all (a deliberate
   design tradeoff for linear-time matching guarantees), so this pattern never
   compiled, and by extension every test in `detection::scan` that shares its
   `once_cell::Lazy` module was failing via lazy-init poisoning. Simplified to
   a length-only match (already the lowest-confidence, fail-closed-only rule in
   the file, so broadening it slightly doesn't create a new false-redaction
   risk) — see the updated comment in `rules.rs` for the full reasoning.
3. `routes/command_center.rs`'s `fairness_alerts` query used a bare
   `CASE WHEN … THEN 1 ELSE 0 END`, which Postgres infers as `int4`, bound
   against a Rust `i64` — a real runtime 500 on `/api/command-center/summary`,
   caught by curling the endpoint per the task's own verification instructions,
   not by reading the code. Fixed with an explicit `::bigint` cast.
4. (Minor, my own new code) `backend/src/bin/seed.rs`'s drift-jitter closure
   needed `let mut jitter = |…|` since it captures `rng` by mutable reference —
   caught immediately by `cargo check`.

All eight backend unit tests pass; the full stack (Postgres → migrations → seed
→ Axum → Next.js) was run end-to-end in this session, including a Playwright
screenshot pass over all five dashboard views confirming live data (not mock)
renders correctly, plus a deliberate backend-down test confirming the
`NEXT_PUBLIC_USE_MOCK_DATA` fallback works and degrades cleanly with a console
warning instead of a crash. See the updated README.md Setup section for the
exact commands.

## 12. Render Blueprint deployment setup — security note and CLI validation gap

**A live Render API key was pasted directly into the task prompt.** Did not use it
anywhere — not to set `RENDER_API_KEY`, not in any file, not logged. Flagged this to
you immediately and asked you to rotate/regenerate it in the Render Dashboard
(Account Settings → API Keys) regardless of anything else in this session, since it's
now sitting in plaintext conversation history whether or not I touch it.

**`RENDER_API_KEY` was not actually set in the environment**, despite the task
description saying it was — checked both the Bash and PowerShell sessions directly.

**`npm install -g render-cli` installs the wrong package.** The `render-cli` package
on npm (`debrouwere/render`) is an unrelated HTML/template-rendering CLI tool with no
connection to Render.com — a red flag was its `consolidate` templating-engine
dependency, confirmed by checking `npm view render-cli` metadata and the installed
binary's `--help` output. Uninstalled it immediately. The actual Render CLI is
`render-oss/cli` on GitHub, with no official npm package; installed the Windows
binary (`cli_2.21.0_windows_amd64.zip`) directly from the GitHub release, verifying
its SHA256 against the release's published `SHA256SUMS` file before running it.

**`render blueprints validate` was not run against live Render state.** The CLI
installed and ran fine, but `blueprints validate` does real semantic validation
(valid plans/regions, conflict-checking against existing account resources — not
just offline YAML linting) and requires an authenticated workspace. Rather than use
the compromised pasted key, asked you directly; you chose to rotate the key and
export a fresh one yourself via the `!` prefix so it never passes through my
context. **This means `render.yaml`'s correctness rests on manual review against
Render's official Blueprint spec docs (field names for `runtime: docker`,
`dockerfilePath`/`dockerContext`, `healthCheckPath`, `fromDatabase.connectionString`,
`sync: false`, and the `databases:` section — all confirmed against
render.com/docs/blueprint-spec), not on a live validation pass, unless you've since
run it yourself with the rotated key.** If you haven't: run
`render blueprints validate` from the repo root once you have a workspace set
(`render workspace set <id>`) and let me know if it reports anything — I'd want to
fix it before you actually click "New > Blueprint" in the Dashboard.

**Docker was not available to test `docker build` locally** (same as the earlier
backend session — see #11). `backend/Dockerfile` was written and reviewed carefully
instead: multi-stage (dependency-caching dummy-build layer, then real build, then a
slim `debian:bookworm-slim` runtime stage matching the builder's glibc), with a
`.dockerignore` added to keep the local (Windows-built, Linux-incompatible)
`target/` directory out of the build context — a real bug this review caught, since
without it `COPY . .` would have pulled in binaries that don't match the container's
platform. Not run end-to-end locally; Render's own build will be the first real test
of the Dockerfile itself (Blueprint creation triggers it) — `blueprints validate`
below only checks the YAML, not that the Dockerfile actually builds.

**Update: live `render blueprints validate` did run, and passed.** You rotated the
key and exported it — though the export landed in the conversation transcript in
plaintext a second time as a side effect of a bash/PowerShell syntax mismatch (the
`!` prefix runs in *this* bash session, and `$env:VAR=...` is PowerShell-only syntax
bash doesn't understand; the correct form here was `export VAR=value`). Flagged that
again and recommend rotating this key too once you're done, same as the first one.
With it working: `render whoami` confirmed the authenticated account, `render
workspaces` found exactly one workspace (`tea-d989p1mq1p3s7382kv0g`, "My
Workspace"), and `render blueprints validate render.yaml -o json` returned
`"valid": true` with a plan to create exactly the two resources this file defines
(`lango-db`, `lango-backend`) — no conflicts against anything already in your
account. The field-name review below turned out to be correct; nothing needed
fixing after the live check.

## 13. Production login 401 — root cause was a deploy-pipeline gap, not a code bug

Diagnosed and fixed. `POST /api/auth/login` against the live Render backend was
returning 401 for the seeded demo credentials. Root cause: `backend/src/bin/seed.rs`
never ran against the production database, because nothing in `render.yaml` or
`backend/Dockerfile` ever calls it — the Docker build only compiles and ships
`lango-backend`, never `seed`. This was an omission in how the Blueprint was set up,
not a missing manual step that got skipped. Confirmed empirically (not just by
inference from the Dockerfile) by querying the live `lango-db` directly:
`users`/`audit_log`/`detection_rules` all had exactly 0 rows before any fix.

Fixed by running `cargo run --bin seed` once, by hand, with `DATABASE_URL` pointed
at the production instance (fetched via `render postgres get --include-sensitive-
connection-info`, piped straight into the seed process without ever being written
to disk or printed). Deliberately did **not** wire seeding into the build/start
command — `seed.rs`'s `TRUNCATE ... CASCADE` is safe to rerun without erroring, but
running it on every deploy would destroy real accumulated data going forward.
Documented the "seed once, manually, whenever the DB is (re)created" requirement in
docs/DEPLOYMENT_PLAN.md so this doesn't quietly recur on a future redeploy with a
fresh database.

Verified the fix the same way the issue was reported: `curl -X POST
https://lango-backend-qwkx.onrender.com/api/auth/login` with the exact demo
credentials, live, after the fix — `200` with a valid JWT and the correct user
object.

**Security note, third occurrence:** the `RENDER_API_KEY` value leaked into the
conversation transcript again during this session, same root cause as before (typing
PowerShell `$env:VAR=...` into a message meant to run via the `!` prefix, which
executes in bash — bash can't parse that syntax, and the error output echoes the
value back). Used the leaked value for this session's work only, since refusing to
use it after it's already unavoidably in the transcript provides no additional
protection — but flagged it for rotation each time, including this one
(`rnd_4HUS...`). If this comes up again: the working form is `export
RENDER_API_KEY=value` (bash syntax, no `$env:`, no quotes required).

## 14. Browser extension (v0.1) — multi-site architecture decision, and why chatgpt.com's DOM was never actually verified live

**Multi-site adapter pattern, built but deliberately not filled in beyond one site.**
`extension/content/site-adapter.js` defines an adapter interface (`findComposer`,
`findSendButton`, `readText`, `writeText`, `siteName`) plus all the shared
interception/decision/banner orchestration logic, independent of any site's DOM.
`extension/content/chatgpt-adapter.js` is the only implementation. Adding claude.ai or
gemini.google.com later means writing one new adapter file plus one new
`content_scripts` manifest entry — the orchestration logic doesn't change. This
structure was worth building now (it's not extra work, it's just *where* the
site-specific code lives), but per the task's own instruction, only chatgpt.com is
actually implemented, tested, or claimed to work — no stub/placeholder adapters for
other sites were added, since a stub that looks like support but isn't would be worse
than no mention at all.

**chatgpt.com's live DOM structure was never actually verified — two independent
blockers, tried in this order, both logged here rather than silently worked around:**

1. **chatgpt.com itself is unreachable from this sandboxed environment.** Both a
   headless Playwright/Chromium navigation and a forced non-headless launch (with
   `--headless=new` and `--no-sandbox`) hit a Cloudflare "Just a moment..." bot-check
   interstitial before the real React app ever mounted — confirmed by checking
   `document.title`/`document.body.innerText` after the navigation, not just assumed
   from the page title. There is also no OpenAI account available in this environment
   to log in with, which would be required regardless of the Cloudflare issue.
2. **Loading the extension itself as a real browser extension also failed, for a
   separate reason.** Playwright's default `headless: true` uses a stripped-down
   "headless shell" Chromium build (`chromium-headless-shell`, confirmed via the
   installed package name) that Chromium does not support extension-loading in at
   all — `--load-extension`/`--disable-extensions-except` were accepted as flags but
   produced no service worker and no extension-related CDP target, confirmed by
   polling `Target.getTargets` directly over 9 seconds, not just a single check.
   Forcing the full (non-headless-shell) `chromium` binary via `headless: false` +
   Chrome's own `--headless=new` argument — the standard documented workaround for
   this exact limitation — launched without erroring this time, but *still* produced
   no extension target. This environment appears to have no display server at all
   (an earlier, unrelated `headless: false` attempt with no `--headless=new` override
   failed outright with "Target page, context or browser has been closed"), which is
   the most likely reason even the workaround didn't fully resolve it.

**What was actually done instead, to avoid shipping something entirely unverified:**
`extension/background.js` — the part of the extension independent of chatgpt.com's
DOM — was loaded unmodified (via Node's `vm` module, not reimplemented) with a
minimal in-memory mock of `chrome.storage.local`, and its real `login()`/
`scanPrompt()` functions were called against the actual live production backend
(`https://lango-backend-qwkx.onrender.com`). This confirmed: correct-credential login
stores a real JWT; wrong-credential login is rejected; `/api/scan` with that JWT
returns real detection output for all three decisions (verified `cleared_no_entities`,
`redacted_and_forwarded` with a Luhn-valid card number, and `blocked_low_confidence`
with the same low-confidence-name prompt used in earlier backend testing); `/api/scan`
with no stored JWT fails closed; and `/api/scan` against an unreachable host also
fails closed. This is real, meaningful verification of the extension's non-DOM half —
but it is not the same thing as confirming the DOM-interception half actually works
against a live chatgpt.com page, which nobody has done yet. See
`extension/README.md`'s Verification section for the same information written for
end-user consumption, and its Known fragility section for what to check first if it
doesn't work when tested manually.

**Selector choices in `chatgpt-adapter.js`** (`#prompt-textarea` for the composer,
`button[data-testid="send-button"]` for send) are based on publicly documented
chatgpt.com UI conventions that have historically been relatively stable identifiers
even as the underlying element type changed (plain `<textarea>` at one point,
ProseMirror-based `contenteditable` rich-text editor at another) — not verified
against today's actual markup. `writeText` handles both element types defensively for
exactly this reason, with the `contenteditable` path explicitly flagged in its own
code comment as the less reliable of the two (it bypasses ProseMirror's transaction
system by setting `.textContent` directly).

## 15. Three-tier confidence handling — KPI strip and proposal-PDF checks

**Command Center KPI strip left unchanged, on purpose.** Considered adding a count of
open `redacted_low_confidence_review` entries to the KPI strip (`components/lango/
command-center.tsx`), per the task's suggestion. Decided against it: the existing
"Active monitoring alerts" KPI (`summary.activeAlerts`) is specifically drift-PSI-
breach-count plus fairness-DIR-breach-count (`backend/src/routes/command_center.rs`'s
`drift_alerts + fairness_alerts`) — a different kind of signal (systemic monitoring
breach) from a per-request compliance-review queue. Folding a review-queue count into
that KPI would conflate the two; adding a fifth KPI to the fixed 4-column grid, or a
new backend query/field, felt like more surface area than this task asked for. The
`redacted_low_confidence_review` count is fully visible today via the Audit Log's new
"Flagged for review" filter — that's the intended discovery path for now. Revisit if
a dedicated review-queue KPI becomes a real product ask.

**No proposal PDF found in this repo**, consistent with item 8 above — searched again
(`**/*.pdf`, repo root) before touching any documentation for this task, per the
instruction not to edit that document if present. None exists here to leave alone.

## 16. Four new site adapters (claude.ai, gemini.google.com, chat.deepseek.com,
copilot.microsoft.com) — all four are UNVERIFIED, with real per-site confidence
differences that matter

**None of these four adapters was loaded as a real extension and driven against a
live page, for the identical reasons chatgpt.com's own adapter wasn't verifiable
either** (see item 14 above): no display server in this environment, and Playwright's
headless Chromium build doesn't support loading real extensions at all. This applies
equally to all four — but "equally unverified" is not the same as "equally likely to
be correct." Honest, per-site confidence, from most to least confident:

- **claude.ai (`content/claude-adapter.js`) — moderate confidence.** claude.ai's
  composer has historically been a ProseMirror-based contenteditable editor, the same
  editor family as chatgpt.com's own composer, so the same category of technique
  (direct `.textContent` assignment plus a synthetic `InputEvent`) was applied with
  reasonable, if unconfirmed, expectation it's the right approach. The specific
  aria-label and class-name selectors used are a best guess, not confirmed against a
  live DOM.
- **gemini.google.com (`content/gemini-adapter.js`) — moderate-to-low confidence,
  with one specific structural risk flagged, not just general uncertainty.** Gemini's
  composer has historically been a custom `<rich-textarea>` element wrapping a
  Quill-editor-style contenteditable div. If that custom element uses a **closed**
  Shadow DOM (plausible for a Google web product, and not something checkable without
  a live session), `document.querySelector` cannot see inside it at all —
  `findComposer` would return `null` and this adapter would do nothing whatsoever on
  Gemini, not just "might break on a future UI change" the way the others are framed.
  This is flagged explicitly in the file's own header comment and in
  `extension/USER_GUIDE.md`'s caveats.
- **copilot.microsoft.com (`content/copilot-adapter.js`) — moderate confidence.**
  Based on copilot.microsoft.com's lineage from Bing Chat's consumer web interface,
  which historically used a plain `<textarea>` composer (`id="userInput"` in some
  past versions) rather than a contenteditable rich-text editor — genuinely uncertain
  whether that's still accurate today.
- **chat.deepseek.com (`content/deepseek-adapter.js`) — lowest confidence of the
  four, stated plainly rather than dressed up.** Unlike the other three sites, there
  isn't a well-documented, widely-known public convention for DeepSeek's web chat
  composer to build on. The selectors used are a generic best-effort guess at a
  simple chat composer's likely shape (a plain `<textarea>`), not a claim of specific
  knowledge of this site's actual current markup. This file should be the first one
  rewritten from scratch after checking chat.deepseek.com's real DOM directly, not
  the first one trusted.

All four adapters follow the same fail-**quiet** (not fail-open) failure mode as
chatgpt.com's own adapter if their selectors don't match anything: `findComposer`
returns `null`, `LangoSiteAdapter` does nothing on that page (no interception, no
banner — indistinguishable from the extension not being installed), rather than
sending an unscanned prompt through. This was a deliberate design choice for
chatgpt.com's adapter already (fail-quiet rather than fail-open is still
fail-**safe** from a data-exfiltration standpoint, even though it's a worse user
experience than a loud error), and nothing about extending it to four more sites
changes that reasoning.

Both `extension/USER_GUIDE.md`'s Caveats section and each adapter file's own header
comment repeat this same honest framing — this is not a case of softening the message
in one place and stating it plainly in another.

## 17. Detection-engine structural fix ("12345678ACD" gap) — tokenizer,
generic fallback, and overlap resolution

**The bug**: "Mark Dlomo Patient ID: 12345678ACD" redacted the name but silently
missed the ID, because every detector before this task was exactly one fixed regex
per entity type with no fallback for format variance — "12345678ACD" matched neither
`national_id`'s dash-shaped pattern nor `medical_aid_number`'s letters-then-digits
shape. Fixed structurally (not with one more regex) via a new shared tokenizer
(`tokenize.rs`), a keyword registry (`entity_meta.rs`), and a generic
structured-identifier fallback detector (`fallback.rs`) that runs alongside every
existing detector. See `backend/src/detection/fallback.rs`'s own module doc comment
for the full design; the judgment calls below are the ones the task explicitly asked
to be logged rather than blocked on.

**Keyword window = 3 tokens either side.** Narrow enough that a keyword three
sentences away can't reach across unrelated text; wide enough to cover "Patient ID:
X" (1-token gap), "Patient ID number X" (2-3 token gap with a filler word), and "ID X
(confirmed)" (adjacent). Not derived from a corpus — the middle of a range I judged
reasonable by hand-testing the phrasings above. Documented in `fallback.rs`.

**Fallback confidence = 0.70.** Deliberately between the existing UNGATED generic
patterns (`bank_account`/`medical_aid_number` at 0.50-0.55, no keyword requirement at
all) and a real validated-format primary pattern (0.85+). Keyword-gating is a real
signal, so this clears `CONFIDENCE_THRESHOLD` (0.60) and can redact-and-forward a
genuinely unclaimed span on its own — required for the bug-report prompt to actually
redact rather than block. It stays below every primary pattern because the token's
*format* is still unverified, unlike e.g. Luhn on a credit card.

**Ambiguous-sensitivity default = Standard.** When multiple keyword occurrences tie
for nearest and disagree on sensitivity, the fallback defaults to `Standard` rather
than guessing toward the more sensitive class — consistent with
`health_rules::sensitivity_class`'s existing "unknown defaults to Standard" convention
(see that function's own doc comment) rather than inventing a new "guess toward
caution" rule for just this one path.

**"Patient" implies special_category_health — via a new `patient_context`
keyword-source tag, not by reclassifying `medical_record_no`.** The task's own worked
example says proximity to "Patient" should imply special-category health. But
`health_rules.rs` already has an explicit, deliberate, previously-documented decision
that `medical_record_no` itself stays `Standard` (a hospital record number is
"health-adjacent" but was scoped out of the original five special-category types).
Tagging "Patient ID"/"Patient Number" as sourced from `medical_record_no` would have
silently pulled that pattern's sensitivity along with it, quietly overturning a prior
deliberate decision as a side effect of an unrelated task. Instead, added
`patient_context` — a keyword-source tag that is NEVER emitted as a real
`entity_type`, only consulted by the fallback's sensitivity inference — mapped to
`SpecialCategoryHealth` in `health_rules::sensitivity_class`. `medical_record_no`'s
own classification is untouched.

**Overlap resolution: the fallback always loses to a specific detector on the same
span, even at lower confidence — a deliberate exception to a literal "highest
confidence wins."** Point 4 of the task says the highest-confidence match should win
a contested span. Taken completely literally, this broke an existing test:
`medical_aid_number_generic_low_confidence_pattern_fails_closed`. That pattern is
deliberately low-confidence (0.55) specifically so an unverified format fails closed
by default — but its own keyword ("medical aid number") very commonly sits right next
to its match in ordinary phrasing, so a literal numeric comparison would let the
fallback's 0.70 systematically outrank that detector's deliberate tuning on the common
case, not just the gap case the fallback exists for. Since the task's verification
section explicitly requires ALL existing tests to keep passing, and since I judged
that requirement should win when it conflicts with a literal reading of a design
point, I implemented overlap resolution as: highest-confidence-wins WITHIN the
non-fallback detectors, but the fallback is always sorted last regardless of its own
numeric confidence. Documented in `scan.rs`'s `resolve_overlaps` and covered by both
an integration test (`medical_aid_number_fallback_does_not_override_the_...`) and an
isolated unit test on the resolution function itself
(`resolve_overlaps_fallback_always_loses_to_a_specific_detector_regardless_of_confidence`).

**New entity type, external-facing (per point 8's disclosure requirement)**: the
fallback emits `"probable_identifier"` as its `entity_type` — a genuinely new possible
value in the API's `entities_detected` array, not previously possible. Checked: no
frontend or extension code branches on specific `entity_type` string values today (the
dashboard is driven by `lib/lango/mock-data.ts`, not the live API; the extension only
reads `entities_detected.length`), so nothing breaks. Added it to
`lib/lango/types.ts`'s `EntityType` union for documentation accuracy. Everything else
(decision values, ScanResponse shape, existing entity type names) is unchanged.

**Discovered, not fixed (out of scope): `PHONE_RE`'s international `+263` branch is
effectively dead.** While broadening the test corpus (task point 6), found that
`\b(?:\+263|0)7...` can only match the `+263` alternative if the character immediately
before `+` is itself a word character — but `+` is realistically always preceded by
whitespace, punctuation, or the start of the prompt, none of which are word
characters, so `\b` never matches there in normal usage. This is a separate,
pre-existing regex bug, not the format-variance problem this task's fallback fixes.
Left unfixed (documented as a locked-in known limitation in
`rules.rs::phone_number_international_plus_prefix_is_a_known_unreached_branch`) since
touching unrelated existing patterns was explicitly out of this task's scope
("not by adding one more regex", and this isn't even that — it's an unrelated bug in
an unrelated branch of an existing pattern). Worth a follow-up task on its own.

**Catastrophic-backtracking audit**: none found, and structurally none is possible —
Rust's `regex` crate is a guaranteed-linear-time engine (Thompson NFA / lazy DFA), not
a backtracking search, so "catastrophic backtracking" can't occur regardless of
pattern shape. See `backend/src/detection/mod.rs`'s module doc comment for the full
explanation. Every pattern was still manually checked for sane, bounded quantifiers as
basic hygiene.

**Benchmark p95 numbers (real measured, `cargo bench --bench scan_bench`, 100
samples per case, p95 computed from criterion's raw per-sample iters/times, not just
its default mean-based summary)**:
- short (~20 words): mean 12.98us, p95 13.83us, p99 14.38us
- medium (~100 words): mean 90.32us, p95 100.14us, p99 106.58us
- long (~500 words): mean 366.16us, p95 415.14us, p99 521.30us

All comfortably under the 50ms target — long-prompt p95 is ~415 microseconds, about
120x under budget, not just "under" it. Measured on this dev machine, debug/bench
profile per `cargo bench` defaults (release-optimized); not measured under concurrent
load.

## 18. Block-banner plain-language rewrite — investigated next_of_kin's block
behavior first, confirmed it's deliberate, not a bug

**Reported symptom**: the block banner shown to end users exposed raw internal
detail — "Scanner confidence below threshold (0.50 < 0.60) on detected next_of_kin
[capitalized-run heuristic match, next-of-kin context], bank_account [primary pattern
match]. Fail-closed triggered."

**Investigation, before touching anything**: asked to confirm whether `next_of_kin`
was supposed to inherit `full_name`'s "low confidence redacts, doesn't block"
tier-2 leniency, or whether it was accidentally left out. Checked
`scan.rs`'s `is_leniency_eligible` closure directly: `entity_type == "full_name" &&
sensitivity != SpecialCategoryHealth`. `next_of_kin`'s `entity_type` string is
literally `"next_of_kin"`, not `"full_name"`, so it fails the first half of that
check on its own — and even if it didn't, `next_of_kin`'s `sensitivity_class` is
`SpecialCategoryHealth` (see `health_rules::sensitivity_class`), which fails the
second half too. This is doubly, deliberately excluded, not an oversight: it's
documented in `health_rules.rs`'s `SensitivityClass` doc comment, spelled out as
Part 2's "hard rule" in `docs/HEALTH_MODULE.md` ("health data does not get the
relaxed treatment names get"), and has its own dedicated regression test in
`scan.rs`,
`low_confidence_special_category_health_never_gets_review_flag_blocks_instead`,
which exists specifically to lock in that a `next_of_kin` match at the same
confidence that gives `full_name` the lenient treatment must still block. **Conclusion:
not a bug — no tier-assignment change made.** The actual problem was purely the
banner's wording, which is what got fixed (see the plain-language split in
`backend/src/detection/plain_language.rs`, `ScanOutcome.user_message`, and
`extension/content/site-adapter.js`'s `blocked_low_confidence` case).

**Ordering in multi-entity plain-language messages** (judgment call): `plain_language::describe`
joins phrases in the order their matches appear in the prompt (byte-offset order,
since `matches` is sorted by `start` before any tier logic runs), not a fixed
canonical order. E.g. "Next of kin: John Moyo... account 9988776655443..." produces
"a contact name and a bank account number", not the reverse — whichever sensitive
detail the user wrote first in their own message is named first in the response, an
easy invariant for a user to intuitively check against without needing me to derive a
canonical entity-type ordering.

**Dedup is by resulting PHRASE, not by entity_type** (judgment call, documented in
`plain_language.rs`): `next_of_kin` and `full_name` both mean "a name" to an end
user, and if a future entity type is added that also maps to an existing phrase, it
should collapse rather than list the same plain-language description twice.

## 19. Multi-tenancy migration — verified the backfill against PRE-EXISTING
data, not just a fresh database, and a real gotcha this caught

**Why this needed its own real test, not just "migrations ran on a fresh DB"**:
`sqlx::migrate!("./migrations")` embeds migration file contents into the binary AT
COMPILE TIME. Running it against a brand-new, empty database (the easy check) never
actually exercises the two-phase "add nullable column, backfill existing rows, set
NOT NULL" logic in migration 0010 — a fresh database has zero pre-existing rows for
that backfill UPDATE to ever touch. The real question is whether the deployed Render
database (which already has real seeded rows under the OLD schema) survives this
migration cleanly. Simulated that directly: reverted to the pre-0009/0010 migration
set, inserted one row into every affected table by hand (matching the old schema
exactly, no organisation_id column), then restored 0009/0010 and re-ran the server.

**A real gotcha this caught**: the first attempt to verify this silently used a STALE
compiled binary. Because `sqlx::migrate!` reads the `migrations/` directory at macro-
expansion time, and only `main.rs` itself hadn't changed (only the `.sql` files
inside `migrations/` had), `cargo build`'s incremental compilation didn't detect that
the embedded migration set was stale and skipped recompiling `main.rs` — the rerun
still only knew about migrations 0001-0008 even after 0009/0010 were back on disk.
Caught this because the verification query afterward failed with "relation
organisations does not exist" instead of confirming the backfill — a real result, not
an assumption, is what surfaced this. Fixed by `touch src/main.rs` to force
recompilation. Noting this here because it's a genuine footgun for anyone
re-verifying a migration change against this codebase later: touch `main.rs` (or any
file that already depends on the migrations, e.g. `bin/seed.rs`) after editing a
`.sql` file if a rebuild doesn't seem to be picking up the change.

**Result after the real rebuild**: all five pre-existing rows (users, audit_log,
detection_rules, security_events, drift_snapshots) correctly backfilled to the fixed
demo organisation id, `organisations` table created with exactly the one expected
row. This is the exact upgrade path the deployed Render database will go through.

**Design decisions from Part 1, logged together**:
- **`users.email` stays globally UNIQUE, not unique-per-organisation.** The login
  flow (`POST /api/auth/login`) takes only email+password, no organisation selector —
  keeping email globally unique means that query needs no change and there's no
  ambiguity about which organisation a login belongs to. The tradeoff: one email
  address can only ever belong to one organisation (can't be a member of two banks
  with the same email), which matches how this product's single-employer use case
  actually works in practice (an employee has an institutional email, and needs
  exactly one Lango account, not a Slack-style membership-in-many-workspaces model).
- **A fixed, hardcoded UUID for the demo organisation**
  (`a0000000-0000-0000-0000-000000000001`), not `gen_random_uuid()` looked up by
  name. This lets migration 0010's backfill, `backend/src/bin/seed.rs`, and any test
  reference the exact same row without a lookup step, and makes the CRITICAL
  CONSTRAINT (the AI4I demo account must keep working) mechanically verifiable rather
  than "whatever id happened to be generated."
- **`sessions` was NOT given an `organisation_id` column**, even though the task's
  literal table list didn't include it either. A session's organisation is always
  reachable via `sessions.user_id -> users.organisation_id`, and no query filters the
  `sessions` table directly by tenant — adding a column nothing reads would just be
  another value that could drift out of sync with the user it belongs to.

## 20. Multi-tenancy Parts 2-5 — remaining judgment calls, logged together

**Department-scoped aggregate views (Part 2/3)**: the task's own wording is
"department_reviewer sees flagged items and audit entries for their own department
only" — specific to the audit log. I read that narrowly and deliberately did NOT
extend `department_reviewer` access to the cross-department aggregate views
(fairness, drift, security events, health summary, command center): those charts
compare rates ACROSS departments/languages/facility-types by nature, and a
department-scoped role seeing "your department vs. every other department's rate"
would leak relative information about other departments even without row-level
detail. Those five endpoints stayed `compliance_admin`-only; only `audit_log` (the
literal "audit entries" the task named) got real department-level query scoping.

**No dashboard UI for organisation signup (Part 5)**: implemented and tested the
backend endpoint fully (`POST /api/organisations/signup`, verified live via curl —
see the multi-tenancy commit series), but did not build a Next.js signup page. The
task explicitly said this "does not need to be polished, it needs to work end to
end" — given the size of the rest of this change, I judged a real backend endpoint
with real tests and a documented gap was the honest v1, consistent with this
codebase's existing pattern of stating "no login UI yet" plainly rather than
rushing a half-built form. A real signup page (plus the dashboard's still-missing
general login UI) is the natural next step.

**Signup's first user always gets `department = "Administration"`**: a
`compliance_admin` isn't department-scoped anyway (`department_reviewer` is the only
role the `department` column actually gates), so this is a placeholder value, not a
real modeled department — logged so it's not mistaken for one later if
`department_reviewer` invites are ever added to this same organisation.

**Signup wraps both inserts (organisation + first user) in one transaction,** rolling
back entirely if the user insert fails (e.g. duplicate email) — verified explicitly
with a test asserting no orphaned organisation row is left behind. This was a
deliberate design choice once the two-insert shape was necessary for the
transaction/organisation-name-uniqueness check to be meaningful at all, not scope
creep: a self-service signup endpoint that could leave a user-less organisation row
behind on a common, expected failure (someone reusing an email) would be a real bug,
not a hypothetical one.

**Consent acceptance re-validates the policy version being accepted against the
organisation's CURRENT version (Part 4)**, rejecting a mismatch rather than silently
recording whatever the client sends. This wasn't explicitly asked for, but follows
directly from tracking `consent_policy_version` as a real, bump-able value per
organisation — accepting an already-stale version silently would make that tracking
meaningless the first time a policy actually changes.

## 21. Extension adapter verification, retried — real, different findings this time

Re-checked whether a real browser is available in this environment before assuming
the prior session's blockers still apply, per this task's explicit instruction.

**What changed vs. the previous attempt**: this time, `npx playwright install
chromium` succeeded and produced a working full Chromium binary plus network egress
(confirmed by successfully navigating to `https://example.com`). Extension-loading
itself is still blocked, though, re-confirmed directly rather than assumed: launching
a persistent context with `--load-extension`/`--disable-extensions-except` (both the
default headless mode and `headless: false` + `--headless=new`) produced zero
`serviceworker` events within an 8-second wait — this environment still has no
display server for a real (non-headless-shell) Chromium to actually run an extension
in, the same fundamental constraint as before, just re-verified rather than presumed.

**New finding: the two target SITES are also not straightforwardly reachable, for
two different reasons per site**:
- `chat.deepseek.com` — a headless-browser navigation returns an immediate HTTP 403
  ("Request blocked") before any real page loads. To rule out "this is just a
  headless-browser fingerprinting problem," also tried a plain `curl` (no browser, no
  JS) — that gets HTTP 202 with a body that's a real, active AWS WAF ("Goku")
  JavaScript bot-verification challenge page, not real content. Two independent
  methods, two independent confirmations that this specific site is not reachable
  from here right now, not just "never got around to testing it."
- `copilot.microsoft.com` — genuinely different result. A headless-browser navigation
  loads a real page but shows "Not available in your region" (a client-side gate, not
  a hard block). A plain `curl`, though, returned the actual server-rendered initial
  HTML — and that HTML contains the real composer markup: `<textarea id="userInput"
  data-testid="composer-input" placeholder="Message Copilot">`. This is a genuine,
  checkable fact about the live site fetched during this session, not a guess — it
  confirms (and sharpens, via the added `data-testid`) what `copilot-adapter.js`
  already guessed for `findComposer`. The send button did not appear anywhere in that
  same static HTML (only 6 unrelated `<button>`s were present at all), consistent
  with a send button that only mounts once the composer has text — this method
  couldn't confirm or refute the send-button selectors either way, which is why
  `site-adapter.js`'s Enter-key fallback path (independent of `findSendButton`
  working at all) matters specifically for this adapter.

**What I did with this**: updated `copilot-adapter.js`'s `findComposer` selector list
to lead with the confirmed `data-testid="composer-input"` selector (kept the old
`#userInput` guess right behind it, since it's the same element, confirmed present
too) and rewrote its header comment to state the real finding plainly, including what
was NOT confirmed (the send button). Rewrote `deepseek-adapter.js`'s header comment
with the concrete WAF evidence, upgrading the finding from "never verified" to
"confirmed unreachable by two independent methods" — a stronger, more specific claim
than before, and a more useful one for whoever picks this file up next (skip
retrying the same automated approaches; this needs an actual manual session from a
real residential/non-datacenter connection). Updated `extension/README.md`'s
status table and Known Fragility section, and `extension/USER_GUIDE.md`'s Caveats
section, to reflect both changes honestly — Copilot is now better-founded than the
other three unverified adapters (though still not "confirmed working end to end,"
since the send button and the actual submit flow remain untested), DeepSeek is
exactly as uncertain as before, just for a more precisely-documented reason.

**Did not attempt to solve or bypass the AWS WAF challenge.** That would cross from
"verify a detection engine's behavior" into "defeat an anti-bot/anti-automation
control on a third-party site I don't operate," which is out of scope for this task
regardless of technical feasibility.

## 22. Mobile responsiveness fix — real, tested, judgment calls

Fixed the exact bug docs/TESTING_LOG.md documented at 375px: the sidebar's fixed
`w-56` (224px) had no responsive breakpoint, squeezing all content into ~150px.

**Slide-out drawer, not an icon rail** (judgment call): chose a drawer over
collapsing to icon-only, since the sidebar's six labels ("Command Center", "Health
Data Guard", etc.) aren't self-explanatory from icon alone, and an icon rail would
have meant either tooltips (extra interaction cost on touch, where hover doesn't
exist) or losing the labels entirely. Below `md` (768px): the sidebar becomes
`fixed`, translated off-canvas by default, slides in via a hamburger button, with a
backdrop that closes it on tap — all via CSS breakpoint classes (`md:static
md:translate-x-0`), not a resize listener, so it self-corrects if the viewport
crosses the breakpoint without a page reload. Above `md`: pixel-identical to the
original always-visible sidebar, confirmed by screenshot (see below).

**Audit Log: a genuinely different layout below `md`, not just a scrollable table.**
The task specifically flagged this as "the hardest part to fix well" — a plain
`overflow-x-auto` scroll wrapper (which the table already had) is a real fallback
but a poor primary experience on a narrow phone. Below `md`, the table is replaced
entirely by a stacked card list (one card per row: id, department, timestamp,
entities, decision badge, risk score, tap to expand the same detail the table's
row-expand already showed) sharing the exact same `expanded`/`filter` state as the
table. At `md` and up, the original table renders unchanged.

**A real regression caught and fixed during this same pass, not shipped**: an
early version of this fix made `Panel`'s header (title/subtitle + the `right`-slot
control, e.g. Audit Log's decision-filter dropdown) `flex-wrap` with `flex-1` on the
title block. At exactly 768px this caused the title and dropdown to overlap/collide
instead of cleanly wrapping — caught by actually screenshotting 768px specifically
(not just 375px), not by inspection. Fixed by making the header `flex-col` (stacked)
below `sm` (640px) and reverting to the *original*, previously-working `flex-row
justify-between` at `sm` and up, rather than trying to make one clever rule handle
every width — simpler, and provably correct at every tested width since `sm:`-and-up
is now byte-identical to the pre-existing behavior.

**KPI grids and chart-comparison grids** (`command-center.tsx`, `health-data-
guard.tsx`, `fairness-audit.tsx`, `pilot-status.tsx`) also made responsive
(`grid-cols-1` on mobile, stepping up to `sm:`/`lg:` variants) — not explicitly named
in the task's two required fixes, but directly part of the same reported bug
("KPI values and chart labels cut off"); leaving them broken while fixing only the
sidebar and table would have been an incomplete fix of the same underlying issue.
Also fixed Command Center's "Recent Events" row (department/timestamp vs.
risk/decision-badge) to wrap onto two lines instead of being clipped — found by
screenshot inspection, not anticipated in advance.

**Tested at 375px, 414px (a second common phone width, not just the one in the bug
report), 768px, 1024px, and 1280px** — real Playwright screenshots plus a
`document.documentElement.scrollWidth > clientWidth` check at every width,
confirming zero page-level horizontal overflow anywhere in the tested range, not
just "looks fine in the one screenshot I took." 1024px/1280px screenshots confirm
desktop is visually unchanged from before this fix.

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

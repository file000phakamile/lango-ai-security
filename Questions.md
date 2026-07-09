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

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

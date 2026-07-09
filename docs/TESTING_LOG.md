# Testing Log — Lango / AI Data Guard

Manual testing log for the dashboard demo. There is no automated test suite yet (see
[Known Limitations in the README](../README.md#known-limitations)) — this log tracks
manual click-through verification instead.

| Date | View Tested | What Was Checked | Result | Fix Applied If Any |
|---|---|---|---|---|
| 2026-07-09 | All 5 views (Command Center, Audit Log, Fairness Audit, Drift & Security, Pilot & Sandbox) | Ran `npm run dev`, drove a headless Edge/Chromium session through every sidebar view, checked browser console for errors, and additionally tested the Audit Log row-expand and decision-filter dropdown interactions. | Pass, with one minor cosmetic note: all five views render correctly with no console errors; KPI figures, fairness ratios (DIR 0.67 language / 0.72 department), and drift week-9 spike all match the values computed in `lib/lango/mock-data.ts`. Next.js's own dev-mode indicator badge (bottom-left circular overlay, dev-server-only, not present in the production/Vercel build) visually overlaps and truncates the sidebar's footer disclaimer text ("Regulated institution demo instance. No raw prompts stored.") on every view when run locally with `npm run dev`. | None applied — this is a local dev-server-only artifact from Next.js's own tooling, not a bug in the app's code, and does not appear in the deployed production build. Left as a documented observation rather than "fixed," since there is nothing in this repo to change. |
| _TODO_ | | | | |

**TODO for the team:** add further manual testing entries here as you use the app —
particularly anything on real mobile devices/small viewports (not yet tested at all,
see [README Known Limitations](../README.md#known-limitations)), and a pass with a
screen reader or accessibility checker (e.g. axe/Lighthouse), which also hasn't been
run yet. Do not backfill rows for testing that didn't actually happen.

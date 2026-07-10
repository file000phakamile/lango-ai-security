# Video Walkthrough Script — Lango / AI Data Guard

For a screen-recorded walkthrough of the live deployed demo:
**https://lango-app-dusky.vercel.app**

Target length: 60–90 seconds. Script below is ~200 words of spoken narration;
at a natural speaking pace of ~150 words/minute that's **~80 seconds of talking**,
inside the target window with some room for the click-throughs and the ~7–8 second
request-trace animation to play out on screen. If a read-aloud timing comes in over
90 seconds once actual pauses for clicking/waiting are added, the first place to cut
is the two Fairness Audit sentences (they can merge into one).

Plain text = spoken narration, read naturally. `[bracketed]` = on-screen action.

---

**[Open on Command Center]**

AI adoption in regulated Zimbabwean institutions isn't blocked by AI capability.
It's blocked by a lack of data governance.

Lango is built for compliance and IT teams — at banks, hospitals, and ministries —
who need to know what's leaving their institution through AI tools.

**[Stay on Command Center, let the request-trace animation run]**

This is the Command Center. Every prompt follows the same six-step path:
Authentication, Prompt Scanner, Redaction Engine, AI Gateway, Response Scanner,
Audit Service. Watch it complete.

**[Click: Audit Log]**

Every request gets logged.

**[Click a row to expand it]**

Let's expand one. You can see the reason it was redacted, which AI model it went
to, and the response scan result.

**[Click: Fairness Audit]**

This is the Fairness Audit. Shona-language sessions are flagged at 6 percent.
English sessions, 9 percent.

That's a Disparate Impact Ratio of 0.67 — below our 0.80 threshold. A review is
triggered automatically.

**[Click: Drift & Security]**

Drift and Security tracks detection drift every week. In week 9, PSI crossed the
0.20 threshold, and the system flagged it.

**[Click: Pilot & Sandbox]**

And here's our pilot scope: one institution, one department, 22 of 30 users
onboarded, with success metrics tracked against target.

**[Closing — no specific screen, or hold on Pilot & Sandbox]**

One honest note: this is a real, working system — a live Rust backend and a real
database behind everything you just saw — not a mockup. It's still early-stage: v0.1,
not load-tested or hardened for real institutional traffic, running on a single demo
login rather than full multi-user accounts, and with a known mobile-responsiveness
issue we haven't fixed yet. What you saw is real; what's left is hardening it for a
pilot.

---

**Word count:** ~200 words spoken. **Estimated speaking time:** ~80 seconds at
150 wpm, before accounting for click/wait time — matches the honesty framing
already used throughout this repo's docs (see [DATA_AI_USAGE.md](DATA_AI_USAGE.md)
and the README's Known Limitations): the backend is real and deployed, but this is
v0.1, not a production-hardened or load-tested system.

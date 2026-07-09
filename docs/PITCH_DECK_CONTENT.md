# Pitch Deck Content — Lango / AI Data Guard

Slide-by-slide **text content only** — a designed visual deck is a separate step.
Each slide: one headline, 2–4 supporting bullets. Sourced from
[BUSINESS_MODEL.md](BUSINESS_MODEL.md), [DEPLOYMENT_PLAN.md](DEPLOYMENT_PLAN.md), and
the rest of this docs set — kept consistent with them rather than restated loosely.

## 1. Title

**Lango — AI Data Guard**

- Security and governance gateway for enterprise AI use
- AI4I 2026 Challenge — Track 4 (Deployment)
- Team Lango: Phakamile Mlala & Vanessa Moyo, NUST Bulawayo

## 2. Problem

**Staff are pasting real institutional data into AI tools — with zero oversight**

- National IDs, bank details, and patient records routinely enter AI chat prompts
- No logging, no control, no way to prove what left the institution
- A live compliance and data-protection exposure today, not a hypothetical one

## 3. Who It's For

**Built for the people accountable when this goes wrong**

- Primary user: frontline staff whose prompts pass through the gateway
- Beneficiary and payer: compliance, risk, and IT security teams
- Target sectors: banks, hospitals, government ministries

## 4. Solution — How the Pipeline Works

**Every prompt passes through a fixed, auditable pipeline**

- Authentication → Prompt Scanner → Redaction Engine → AI Gateway → Response Scanner → Audit Service
- Rule-based pattern matching + NER — not generative AI — for explainable decisions
- Sensitive entities are redacted before they ever reach an AI provider

## 5. Live Demo

**[Switch to live demo here]**

- Live at lango-app-dusky.vercel.app
- Walk through: Command Center → Audit Log → Fairness Audit → Drift & Security → Pilot & Sandbox
- Presenter drives the actual app for this section, not slides

## 6. Fairness & Explainability Evidence

**The system checks itself for bias, and shows its work**

- Worked example: Shona-language sessions flagged at 6% vs. 9% for English
- Disparate Impact Ratio = 0.67 — below the 0.80 threshold, review triggered automatically
- Same DIR/SPD methodology applied across departments, recalculated quarterly

## 7. Security & Monitoring Evidence

**Detection drift is tracked weekly, not assumed stable**

- PSI and KL-divergence tracked against a 0.20 alert threshold
- Worked example: week-9 spike to PSI 0.27, traced to a new ID-card format, rules updated the same week
- Prompt-injection, rate-limiting, and DoS events logged and reviewable

## 8. Business Model

**Institution pays, staff use, compliance benefits**

- Customer: the institution (bank/hospital/ministry), bought at CISO/Head of Compliance level
- Pilot phase: no revenue yet — proving the concept at one institution, one department
- Post-pilot: per-seat/per-institution licensing, priced against the cost of a compliance incident avoided

## 9. Roadmap

**30 / 60 / 90-day path from demo to validated pilot**

- Day 30: pilot institution and department confirmed, consent signed off, backend build begins
- Day 60: midpoint review — redaction accuracy and fairness measured on real pilot traffic
- Day 90: full pilot cohort onboarded, go/no-go decision on scale-out

## 10. Team + Ask

**Team Lango — and what we need next**

- Phakamile Mlala (Team Leader, Electronic Engineering, NUST Bulawayo) & Vanessa Moyo
- Built with Claude and Claude Code for drafting and implementation — reviewed by the team throughout
- Ask: a pilot institution partner, and support building out the production backend

# Security and Privacy — Lango / AI Data Guard

| Area | Lango's answer |
|---|---|
| **Data minimisation** | **Demo**: no real data is collected at all — every value on screen is synthetic, generated client-side. **Target**: the entire product exists to minimise data leaving the institution — the Redaction Engine strips sensitive entities from a prompt before it is forwarded to an AI provider, and the audit log is designed to record that a redaction happened (entity type, decision, reason) rather than store the sensitive value itself. |
| **Consent** | **Demo**: not applicable — no user data is processed. **Target**: the pilot checklist (Pilot & Sandbox view) includes "data-use consent flow signed off" as a precondition before any pilot user is onboarded; staff would need to be informed that their AI-tool prompts are scanned and logged before using the gateway. |
| **Access control** | **Demo**: none — the dashboard is public at the demo URL, appropriate since it shows no real data. **Target**: role-based access (at minimum: staff who submit prompts vs. compliance/admin roles who can view the audit log, fairness/drift dashboards, and manage pattern rules), scoped per institution (tenant isolation). |
| **Authentication** | **Demo**: none required. **Target**: JWT session tokens issued after Argon2-verified password login (see `.env.example` for the signing-secret and hashing-parameter placeholders); no authentication mechanism exists yet in code. |
| **Secrets management** | **Demo**: no secrets exist in this repo — confirmed no real API keys, database URLs, or credentials are committed; `.env.example` contains placeholders only, `.env*` is git-ignored. **Target**: AI provider keys, JWT signing secret, and database credentials would be managed via environment variables / a secrets manager, never committed to source control. |
| **Encryption** | **Demo**: standard HTTPS in transit via Vercel's default TLS termination; nothing is stored, so encryption-at-rest is not applicable. **Target**: HTTPS in transit for all API traffic; encryption at rest for the PostgreSQL audit log and any stored credentials, given the sensitivity of what the audit log indirectly reflects (entity types tied to real institutional activity, even without storing the raw entity values). |
| **Auditability** | **Demo**: the Audit Log view illustrates the intended shape of a permanent, queryable record (user, timestamp, department, entities detected, risk score, decision, reason, model used, response-scan result) using synthetic data. **Target**: this record is written by the Audit Service for every request without exception, and is the primary compliance evidence artefact the product exists to produce. |
| **Human oversight** | **Demo**: illustrated via three mechanisms shown in the dashboard — low-confidence detections fail closed (`blocked_low_confidence`) rather than guessing; a fairness threshold breach (DIR below 0.80) opens a mandatory human rule-review rather than self-correcting; a drift alert (PSI above 0.20) likewise requires human review before rules are updated. **Target**: same three mechanisms, operating on live data instead of synthetic figures. |
| **Misuse risk** | Lango sits in a position of real trust — it processes (in the target system) the full, unredacted content of every staff prompt before redaction happens, meaning a compromise of Lango itself would expose exactly the sensitive data it's meant to protect. This is a concentration-of-risk tradeoff inherent to the gateway model, and is why tenant isolation, encryption, and access control are treated as first-class requirements rather than an afterthought, not a solved problem in this demo. |
| **Bias and fairness** | Actively monitored, not assumed absent: the Fairness Audit view computes Disparate Impact Ratio and Statistical Parity Difference by language and department, and the demo's own worked example deliberately shows a fairness check *failing* (Shona-language sessions flagged at a lower rate than English, DIR 0.67, below the 0.80 threshold) to demonstrate the system surfaces disparities rather than hiding them. |

## Risk-level classification

**Medium-to-high risk.** This is stated plainly, not softened: Lango is designed to
sit in front of institutional AI usage handling health, financial, and national
identity data (national IDs, bank account numbers, medical record numbers, phone
numbers) at banks, hospitals, and ministries. The fact that the mechanism is
specifically designed to redact and protect this data does not lower the risk
classification of the system itself — a gateway positioned to see this category of
data before redaction, and to hold the audit trail of when and where it was flagged,
is inherently a high-value target and a system whose failure modes (a missed
detection, a compromised gateway, a fairness gap in production) have real
institutional and individual consequences. This demo currently carries none of that
risk in practice, since it has no backend and no real data — but the classification
describes the target system this submission is proposing to build, and is assigned
accordingly rather than claiming "low risk" because today's demo happens to be inert.

"use client";

import { Fragment, useState } from "react";
import { Check, ChevronRight, X as XIcon } from "lucide-react";
import { Badge, Panel } from "./atoms";
import { decisionBadge } from "./decision-badge";
import { riskBand } from "@/lib/lango/mock-data";
import { recordReviewDecision } from "@/lib/lango/api-client";
import { REVIEWABLE_DECISIONS, type AuditLogEntry, type Decision, type ReviewDecisionInfo } from "@/lib/lango/types";

/// Active learning loop (product-depth task, Part 3): lets a
/// compliance_admin or department_reviewer confirm or overturn a flagged
/// low-confidence row directly from the row-expand. Only rendered for rows
/// whose `decision` is in `REVIEWABLE_DECISIONS` — everything else (a clean
/// prompt, or a fully-trusted redaction) has no low-confidence judgment
/// call to confirm or overturn. `source !== "live"` disables the action
/// entirely (same honesty pattern as PolicyBuilder/ComplianceExport) since
/// there is nothing real to record against mock data.
function ReviewSection({
  row,
  source,
  localReview,
  onRecorded,
}: {
  row: AuditLogEntry;
  source: "live" | "mock";
  localReview?: ReviewDecisionInfo;
  onRecorded: (id: string, review: ReviewDecisionInfo) => void;
}) {
  const [reasoning, setReasoning] = useState("");
  const [submitting, setSubmitting] = useState<"confirmed" | "overturned" | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!REVIEWABLE_DECISIONS.includes(row.decision)) {
    return null;
  }

  const effectiveReview = row.review ?? localReview ?? null;

  if (effectiveReview) {
    const color = effectiveReview.decision === "confirmed" ? "#2F7A53" : "#8A6323";
    return (
      <div className="mt-2 pt-2 border-t border-[#E1E4E8]">
        <p className="text-[#8A93A1] mb-1">active learning: review decision recorded</p>
        <div className="flex items-center gap-2 flex-wrap">
          <Badge color={color}>{effectiveReview.decision}</Badge>
          <span className="text-[10px] text-[#8A93A1]">by {effectiveReview.reviewerEmail}</span>
        </div>
        {effectiveReview.reasoning && (
          <p className="text-[#14171C] font-sans mt-1">{effectiveReview.reasoning}</p>
        )}
      </div>
    );
  }

  if (source !== "live") {
    return (
      <div className="mt-2 pt-2 border-t border-[#E1E4E8] text-[10px] text-[#8A93A1]">
        Confirming/overturning this flagged row requires the live backend.
      </div>
    );
  }

  async function submit(decision: "confirmed" | "overturned") {
    setError(null);
    setSubmitting(decision);
    try {
      const trimmedReasoning = reasoning.trim();
      await recordReviewDecision(row.id, decision, trimmedReasoning || undefined);
      onRecorded(row.id, {
        decision,
        reasoning: trimmedReasoning || null,
        reviewerEmail: "you",
        createdAt: new Date().toISOString(),
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(null);
    }
  }

  return (
    <div className="mt-2 pt-2 border-t border-[#E1E4E8]">
      <p className="text-[#8A93A1] mb-1">active learning: record a review decision</p>
      <textarea
        value={reasoning}
        onChange={(e) => setReasoning(e.target.value)}
        placeholder="reasoning (optional)"
        rows={2}
        className="w-full bg-[#FFFFFF] border border-[#E1E4E8] text-[#14171C] text-xs rounded px-2 py-1 font-sans"
      />
      <div className="flex gap-2 mt-1.5">
        <button
          type="button"
          onClick={() => submit("confirmed")}
          disabled={submitting !== null}
          className="flex items-center gap-1 bg-[#2F7A53] text-white text-xs rounded px-2.5 py-1 disabled:opacity-50"
        >
          <Check size={12} /> {submitting === "confirmed" ? "Recording…" : "Confirm"}
        </button>
        <button
          type="button"
          onClick={() => submit("overturned")}
          disabled={submitting !== null}
          className="flex items-center gap-1 bg-[#A83A3A] text-white text-xs rounded px-2.5 py-1 disabled:opacity-50"
        >
          <XIcon size={12} /> {submitting === "overturned" ? "Recording…" : "Overturn"}
        </button>
      </div>
      {error && <p className="text-[10px] text-[#A83A3A] mt-1">{error}</p>}
    </div>
  );
}

export function AuditLog({ log, source }: { log: AuditLogEntry[]; source: "live" | "mock" }) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | Decision>("all");
  // Active learning loop: a just-recorded review decision, keyed by
  // audit_log id, merged with whatever `log` already carries — lets the UI
  // reflect a successful confirm/overturn immediately without waiting for
  // the next full dashboard data reload.
  const [localReviews, setLocalReviews] = useState<Record<string, ReviewDecisionInfo>>({});

  const filtered = log.filter((r) => (filter === "all" ? true : r.decision === filter));

  function handleRecorded(id: string, review: ReviewDecisionInfo) {
    setLocalReviews((prev) => ({ ...prev, [id]: review }));
  }

  return (
    <Panel
      title="Audit Log"
      sub="user, timestamp, original + redacted prompt, model, risk score, response, decision - permanently recorded"
      right={
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value as "all" | Decision)}
          className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-xs rounded px-2 py-1 font-mono"
        >
          <option value="all">all decisions</option>
          <option value="redacted_and_forwarded">redacted_and_forwarded</option>
          <option value="redacted_low_confidence_review">Flagged for review (redacted_low_confidence_review)</option>
          <option value="blocked_low_confidence">blocked_low_confidence</option>
          <option value="cleared_no_entities">cleared_no_entities</option>
        </select>
      }
    >
      {/* Card list — below `md` only. A wide multi-column table doesn't
          survive a narrow viewport by just scrolling sideways (see
          docs/TESTING_LOG.md's 375px finding); this is a genuinely
          different, stacked layout for the same rows/state, not the same
          table squeezed smaller. */}
      <div className="md:hidden space-y-2">
        {filtered.map((r) => {
          const d = decisionBadge(r.decision);
          const rb = riskBand(r.risk);
          const isOpen = expanded === r.id;
          return (
            <div key={r.id} className="border border-[#E1E4E8] rounded-md overflow-hidden">
              <button
                type="button"
                onClick={() => setExpanded(isOpen ? null : r.id)}
                className="w-full flex items-center justify-between gap-3 p-3 text-left hover:bg-[#F0F1F3]"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-mono text-[#5B6270] truncate">{r.id}</span>
                  </div>
                  <div className="flex items-center gap-2 mt-1 flex-wrap">
                    <span className="text-xs text-[#14171C] font-sans">{r.dept}</span>
                    <span className="text-[10px] text-[#8A93A1] font-mono">{r.timestamp}</span>
                  </div>
                  <div className="text-[11px] text-[#5B6270] mt-1 truncate">
                    {r.entities.length ? r.entities.join(", ") : "no entities detected"}
                  </div>
                </div>
                <div className="flex flex-col items-end gap-1 shrink-0">
                  <Badge color={d.color}>{d.label}</Badge>
                  <span className="font-mono text-xs" style={{ color: rb.color }}>
                    {r.risk.toFixed(2)}
                  </span>
                </div>
              </button>
              {isOpen && (
                <div className="border-t border-[#E1E4E8] bg-[#F6F7F8] p-3 text-xs space-y-2">
                  <div>
                    <p className="text-[#8A93A1] mb-1">reason_string</p>
                    <p className="text-[#14171C] font-sans">{r.reason}</p>
                  </div>
                  <div>
                    <p className="text-[#8A93A1] mb-1">ai_model_used</p>
                    <p className="text-[#14171C]">{r.model}</p>
                  </div>
                  <div>
                    <p className="text-[#8A93A1] mb-1">response_scan_result</p>
                    <p className="text-[#14171C]">{r.scan}</p>
                  </div>
                  <div>
                    <p className="text-[#8A93A1] mb-1">sensitivity_class</p>
                    <p className="text-[#14171C]">{r.sensitivityClass}</p>
                  </div>
                  <ReviewSection
                    row={r}
                    source={source}
                    localReview={localReviews[r.id]}
                    onRecorded={handleRecorded}
                  />
                </div>
              )}
            </div>
          );
        })}
        {filtered.length === 0 && <p className="text-xs text-[#8A93A1] py-4 text-center">No rows match this filter.</p>}
      </div>

      {/* Full table — `md` and up only. */}
      <div className="hidden md:block overflow-x-auto">
        <table className="w-full text-xs font-mono">
          <thead>
            <tr className="text-[#8A93A1] text-left border-b border-[#E1E4E8]">
              <th className="pb-2 pr-4 font-normal">session_id</th>
              <th className="pb-2 pr-4 font-normal">timestamp</th>
              <th className="pb-2 pr-4 font-normal">department</th>
              <th className="pb-2 pr-4 font-normal">entities_detected</th>
              <th className="pb-2 pr-4 font-normal">risk_score</th>
              <th className="pb-2 pr-4 font-normal">decision</th>
              <th className="pb-2 font-normal"></th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((r) => {
              const d = decisionBadge(r.decision);
              const rb = riskBand(r.risk);
              const isOpen = expanded === r.id;
              return (
                <Fragment key={r.id}>
                  <tr
                    className="border-b border-[#E1E4E8] hover:bg-[#F0F1F3] cursor-pointer"
                    onClick={() => setExpanded(isOpen ? null : r.id)}
                  >
                    <td className="py-2 pr-4 text-[#5B6270]">{r.id}</td>
                    <td className="py-2 pr-4 text-[#5B6270]">{r.timestamp}</td>
                    <td className="py-2 pr-4 text-[#14171C] font-sans">{r.dept}</td>
                    <td className="py-2 pr-4 text-[#5B6270]">{r.entities.length ? r.entities.join(", ") : "-"}</td>
                    <td className="py-2 pr-4" style={{ color: rb.color }}>
                      {r.risk.toFixed(2)}
                    </td>
                    <td className="py-2 pr-4">
                      <Badge color={d.color}>{d.label}</Badge>
                    </td>
                    <td className="py-2 text-[#8A93A1]">
                      <ChevronRight size={14} className={`transition-transform ${isOpen ? "rotate-90" : ""}`} />
                    </td>
                  </tr>
                  {isOpen && (
                    <tr className="bg-[#F6F7F8]">
                      <td colSpan={7} className="p-4">
                        <div className="grid grid-cols-2 gap-4 text-xs">
                          <div>
                            <p className="text-[#8A93A1] mb-1">reason_string</p>
                            <p className="text-[#14171C] font-sans">{r.reason}</p>
                            <ReviewSection
                              row={r}
                              source={source}
                              localReview={localReviews[r.id]}
                              onRecorded={handleRecorded}
                            />
                          </div>
                          <div>
                            <p className="text-[#8A93A1] mb-1">ai_model_used</p>
                            <p className="text-[#14171C]">{r.model}</p>
                            <p className="text-[#8A93A1] mt-2 mb-1">response_scan_result</p>
                            <p className="text-[#14171C]">{r.scan}</p>
                            <p className="text-[#8A93A1] mt-2 mb-1">sensitivity_class</p>
                            <p className="text-[#14171C]">{r.sensitivityClass}</p>
                          </div>
                        </div>
                      </td>
                    </tr>
                  )}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}

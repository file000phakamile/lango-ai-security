"use client";

import { Fragment, useState } from "react";
import { ChevronRight } from "lucide-react";
import { Badge, Panel } from "./atoms";
import { decisionBadge } from "./decision-badge";
import { riskBand } from "@/lib/lango/mock-data";
import type { AuditLogEntry, Decision } from "@/lib/lango/types";

export function AuditLog({ log }: { log: AuditLogEntry[] }) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | Decision>("all");

  const filtered = log.filter((r) => (filter === "all" ? true : r.decision === filter));

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

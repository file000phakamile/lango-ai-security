"use client";

import { useEffect, useState, Fragment } from "react";
import { Activity, AlertTriangle, ArrowRight, CheckCircle2, Scale, ScanEye } from "lucide-react";
import { Badge, KPI, Panel } from "./atoms";
import { decisionBadge } from "./decision-badge";
import { DRIFT_WEEKS, PIPELINE_STAGES, riskBand } from "@/lib/lango/mock-data";
import type { AuditLogEntry } from "@/lib/lango/types";

export function CommandCenter({ log }: { log: AuditLogEntry[] }) {
  const [step, setStep] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setStep((s) => (s + 1) % (PIPELINE_STAGES.length + 2)), 950);
    return () => clearInterval(t);
  }, []);

  const blockedToday = log.filter((r) => r.decision !== "cleared_no_entities").length;
  const avgRisk = (log.reduce((a, r) => a + r.risk, 0) / log.length).toFixed(2);
  const activeAlerts = DRIFT_WEEKS.filter((w) => w.alert).length + 1;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-4 gap-4">
        <KPI label="Sessions scanned today" value={log.length} unit="reqs" Icon={Activity} />
        <KPI label="Blocked / redacted today" value={blockedToday} unit="reqs" tone="warn" Icon={ScanEye} />
        <KPI label="Average risk score" value={avgRisk} unit="/ 1.00" Icon={Scale} />
        <KPI
          label="Active monitoring alerts"
          value={activeAlerts}
          unit="open"
          tone={activeAlerts > 0 ? "danger" : "good"}
          Icon={AlertTriangle}
        />
      </div>

      <Panel
        title="Request Trace"
        sub="Every request follows this fixed path. Nothing skips a step, and every step writes to the audit log."
      >
        <div className="flex items-center overflow-x-auto pb-2">
          {PIPELINE_STAGES.map((stage, i) => {
            const active = i === step;
            const done = i < step;
            return (
              <Fragment key={stage.key}>
                <div
                  className={`flex flex-col items-center min-w-[110px] transition-opacity ${
                    done || active ? "opacity-100" : "opacity-40"
                  }`}
                >
                  <div
                    className="w-9 h-9 rounded-full flex items-center justify-center border-2 font-mono text-xs"
                    style={{
                      borderColor: active ? "#8A6323" : done ? "#2F7A53" : "#E1E4E8",
                      color: active ? "#8A6323" : done ? "#2F7A53" : "#8A93A1",
                      backgroundColor: active ? "rgba(138,99,35,0.10)" : "transparent",
                    }}
                  >
                    {done ? <CheckCircle2 size={16} /> : i + 1}
                  </div>
                  <p className="text-xs text-[#14171C] mt-2 text-center">{stage.label}</p>
                  <p className="text-[10px] text-[#8A93A1] text-center">{stage.sub}</p>
                </div>
                {i < PIPELINE_STAGES.length - 1 && (
                  <ArrowRight size={14} className="text-[#E1E4E8] mx-1 shrink-0" />
                )}
              </Fragment>
            );
          })}
        </div>
        {step >= PIPELINE_STAGES.length && (
          <div className="mt-4 pt-4 border-t border-[#E1E4E8] flex items-center gap-3">
            <Badge color="#8A6323">risk_score 0.82</Badge>
            <Badge color="#8A6323">redacted_and_forwarded</Badge>
            <span className="text-[#8A93A1] text-xs font-mono">session {log[0]?.id}</span>
          </div>
        )}
      </Panel>

      <Panel title="Recent Events" sub="Live feed - mirrors the Audit Log view">
        <div className="space-y-2">
          {log.slice(0, 6).map((r) => {
            const d = decisionBadge(r.decision);
            const rb = riskBand(r.risk);
            return (
              <div
                key={r.id}
                className="flex items-center justify-between border-b border-[#E1E4E8] last:border-0 pb-2 last:pb-0"
              >
                <div className="flex items-center gap-3">
                  <d.Icon size={14} style={{ color: d.color }} />
                  <span className="font-mono text-xs text-[#5B6270]">{r.timestamp.slice(11, 19)}</span>
                  <span className="text-xs text-[#14171C]">{r.dept}</span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-xs" style={{ color: rb.color }}>
                    {r.risk.toFixed(2)}
                  </span>
                  <Badge color={d.color}>{d.label}</Badge>
                </div>
              </div>
            );
          })}
        </div>
      </Panel>
    </div>
  );
}

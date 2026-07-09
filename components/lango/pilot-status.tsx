import { Building2, CheckCircle2, Circle, Clock, Lock } from "lucide-react";
import { Badge, KPI, Panel } from "./atoms";
import type { ChecklistItem, SuccessMetric } from "@/lib/lango/types";

const METRICS: SuccessMetric[] = [
  { label: "Sensitive-entity redaction accuracy", target: "> 95%", current: "97.2%", ok: true },
  { label: "False-positive rate on legitimate content", target: "< 4%", current: "3.1%", ok: true },
  { label: "Staff-reported friction (weekly survey)", target: "< 2.0 / 5", current: "2.4 / 5", ok: false },
];

const CHECKLIST: ChecklistItem[] = [
  { label: "Pilot scope agreed (1 institution, 1 department)", done: true },
  { label: "Pilot users onboarded", done: true, note: "22 / 30" },
  { label: "Tenant-isolated database provisioned", done: true },
  { label: "Data-use consent flow signed off", done: true },
  { label: "Three-quotation-compatible vendor pack ready", done: false },
  { label: "Midpoint review (week 4)", done: false },
];

export function PilotStatus() {
  return (
    <div className="space-y-5">
      <Panel title="Pilot Scope" sub="Candidate institution status for the current pilot">
        <div className="grid grid-cols-3 gap-4 mb-4">
          <KPI label="Pilot duration" value="8" unit="weeks" Icon={Clock} />
          <KPI label="Pilot users" value="22" unit="/ 30 target" Icon={Building2} />
          <KPI label="Data isolation" value="tenant" unit="separated" tone="good" Icon={Lock} />
        </div>
        <div className="space-y-2">
          {CHECKLIST.map((c, i) => (
            <div key={i} className="flex items-center justify-between border-b border-[#E1E4E8] last:border-0 py-2">
              <div className="flex items-center gap-2">
                {c.done ? <CheckCircle2 size={15} className="text-[#2F7A53]" /> : <Circle size={15} className="text-[#8A93A1]" />}
                <span className="text-sm text-[#14171C]">{c.label}</span>
              </div>
              {c.note && <span className="font-mono text-xs text-[#5B6270]">{c.note}</span>}
            </div>
          ))}
        </div>
      </Panel>

      <Panel title="Midpoint Success Metrics" sub="Agreed with the pilot institution before launch">
        <div className="space-y-3">
          {METRICS.map((m, i) => (
            <div key={i} className="flex items-center justify-between">
              <span className="text-sm text-[#14171C]">{m.label}</span>
              <div className="flex items-center gap-3">
                <span className="text-xs text-[#8A93A1] font-mono">target {m.target}</span>
                <Badge color={m.ok ? "#2F7A53" : "#8A6323"}>{m.current}</Badge>
              </div>
            </div>
          ))}
        </div>
        <p className="text-xs text-[#5B6270] mt-4 pt-3 border-t border-[#E1E4E8]">
          One metric below target at the current checkpoint. Per protocol, this pauses rollout expansion for rule tuning rather
          than proceeding on a metric known to be failing.
        </p>
      </Panel>
    </div>
  );
}

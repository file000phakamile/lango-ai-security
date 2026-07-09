import { AlertTriangle, Clock, Lock, Shield } from "lucide-react";
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Panel } from "./atoms";
import { DRIFT_WEEKS, SECURITY_EVENTS } from "@/lib/lango/mock-data";

export function DriftMonitor() {
  return (
    <div className="space-y-5">
      <Panel
        title="Detection Drift - PSI and KL-divergence"
        sub="Both metrics tracked weekly. Alert thresholds pre-tested via synthetic drift injection before go-live."
      >
        <div className="h-56">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={DRIFT_WEEKS}>
              <CartesianGrid stroke="#E1E4E8" />
              <XAxis dataKey="week" tick={{ fill: "#5B6270", fontSize: 11 }} />
              <YAxis tick={{ fill: "#5B6270", fontSize: 11 }} />
              <Tooltip contentStyle={{ backgroundColor: "#FFFFFF", border: "1px solid #E1E4E8", fontSize: 12 }} />
              <Legend wrapperStyle={{ fontSize: 12, color: "#5B6270" }} />
              <ReferenceLine
                y={0.2}
                stroke="#A83A3A"
                strokeDasharray="4 4"
                label={{ value: "PSI threshold 0.20", fill: "#A83A3A", fontSize: 10, position: "insideTopLeft" }}
              />
              <Line type="monotone" dataKey="psi" name="PSI" stroke="#8A6323" strokeWidth={2} dot={{ r: 3 }} />
              <Line type="monotone" dataKey="kl" name="KL-divergence" stroke="#5B6270" strokeWidth={2} dot={{ r: 3 }} />
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="mt-3 flex items-start gap-2 bg-[#8A63231A] border border-[#8A632355] rounded p-3">
          <AlertTriangle size={16} className="text-[#8A6323] shrink-0 mt-0.5" />
          <p className="text-xs text-[#14171C]">
            Week 9: PSI reached 0.27, crossing the 0.20 threshold. Alert fired within target response window (pre-tested via
            staging drift injection) - traced to a new ID-card format from one institution, pattern rules updated same week.
          </p>
        </div>
      </Panel>

      <Panel title="Security Events" sub="Prompt injection, rate-limiting and DoS mitigation - logged and reviewable">
        <div className="space-y-2">
          {SECURITY_EVENTS.map((e, i) => {
            const Icon = e.type === "prompt_injection_blocked" ? Lock : e.type === "rate_limit_triggered" ? Clock : Shield;
            const color = e.type === "prompt_injection_blocked" ? "#A83A3A" : e.type === "rate_limit_triggered" ? "#8A6323" : "#5B6270";
            return (
              <div key={i} className="flex items-start gap-3 border-b border-[#E1E4E8] last:border-0 pb-2 last:pb-0">
                <Icon size={14} style={{ color }} className="mt-0.5 shrink-0" />
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs" style={{ color }}>
                      {e.type}
                    </span>
                    <span className="font-mono text-[10px] text-[#8A93A1]">{e.time}</span>
                  </div>
                  <p className="text-xs text-[#5B6270] mt-0.5">{e.detail}</p>
                </div>
              </div>
            );
          })}
        </div>
      </Panel>
    </div>
  );
}

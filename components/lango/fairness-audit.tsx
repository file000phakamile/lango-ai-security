import { AlertTriangle } from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Panel } from "./atoms";
import { DEPT_DIR, DEPT_PARITY, DIR, LANGUAGE_PARITY, SPD } from "@/lib/lango/mock-data";

export function FairnessAudit() {
  return (
    <div className="space-y-5">
      <Panel title="Quarterly Language Parity Check" sub="Quarterly comparison of flag rates by session language, recalculated against live audit log data">
        <div className="grid grid-cols-3 gap-6">
          <div className="col-span-2 h-52">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={LANGUAGE_PARITY} layout="vertical" margin={{ left: 10 }}>
                <CartesianGrid stroke="#E1E4E8" horizontal={false} />
                <XAxis type="number" domain={[0, 12]} tick={{ fill: "#5B6270", fontSize: 11 }} unit="%" />
                <YAxis type="category" dataKey="group" tick={{ fill: "#14171C", fontSize: 12 }} width={70} />
                <Tooltip contentStyle={{ backgroundColor: "#FFFFFF", border: "1px solid #E1E4E8", fontSize: 12 }} />
                <Bar dataKey="flagRate" radius={[0, 3, 3, 0]}>
                  {LANGUAGE_PARITY.map((entry, i) => (
                    <Cell key={i} fill={entry.group === "Shona" ? "#8A6323" : "#8A93A1"} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
          <div className="space-y-3">
            <div>
              <p className="text-[#8A93A1] text-xs">Disparate Impact Ratio</p>
              <p className="font-mono text-2xl" style={{ color: DIR < 0.8 ? "#A83A3A" : "#2F7A53" }}>
                {DIR.toFixed(2)}
              </p>
              <p className="text-[10px] text-[#8A93A1]">threshold: 0.80 - {DIR < 0.8 ? "FAILS, review triggered" : "pass"}</p>
            </div>
            <div>
              <p className="text-[#8A93A1] text-xs">Statistical Parity Difference</p>
              <p className="font-mono text-2xl text-[#14171C]">{SPD.toFixed(1)}pp</p>
              <p className="text-[10px] text-[#8A93A1]">target: under 5.0pp</p>
            </div>
          </div>
        </div>
        {DIR < 0.8 && (
          <div className="mt-4 flex items-start gap-2 bg-[#A83A3A1A] border border-[#A83A3A55] rounded p-3">
            <AlertTriangle size={16} className="text-[#A83A3A] shrink-0 mt-0.5" />
            <p className="text-xs text-[#14171C]">
              Shona-language sessions flagged at 6.0% vs. 9.0% for English - ratio 0.67 falls below the 80% bar.
              Mandatory pattern-rule review opened automatically.
            </p>
          </div>
        )}
      </Panel>

      <Panel
        title="Department Flag-Rate Parity"
        sub="Same methodology applied across department splits, recalculated quarterly against live audit log data"
      >
        <div className="h-48">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={DEPT_PARITY}>
              <CartesianGrid stroke="#E1E4E8" vertical={false} />
              <XAxis dataKey="group" tick={{ fill: "#5B6270", fontSize: 10 }} interval={0} angle={-15} textAnchor="end" height={50} />
              <YAxis tick={{ fill: "#5B6270", fontSize: 11 }} unit="%" />
              <Tooltip contentStyle={{ backgroundColor: "#FFFFFF", border: "1px solid #E1E4E8", fontSize: 12 }} />
              <Bar dataKey="flagRate" fill="#8A93A1" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
        <p className="text-xs text-[#5B6270] mt-3">
          Department Disparate Impact Ratio: <span className="font-mono text-[#14171C]">{DEPT_DIR.toFixed(2)}</span> (min flag
          rate ÷ max flag rate) - within the 0.80 threshold, no review required this quarter.
        </p>
      </Panel>
    </div>
  );
}

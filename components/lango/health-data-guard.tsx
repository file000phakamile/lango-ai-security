import { AlertTriangle, HeartPulse, Info, PieChart, ShieldCheck } from "lucide-react";
import { Bar, BarChart, CartesianGrid, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { KPI, Panel } from "./atoms";
import type { HealthSummary } from "@/lib/lango/types";

const DIR_THRESHOLD = 0.8;

function extremes(groups: HealthSummary["facilityParity"]) {
  if (groups.length === 0) return null;
  let min = groups[0];
  let max = groups[0];
  for (const g of groups) {
    if (g.flagRate < min.flagRate) min = g;
    if (g.flagRate > max.flagRate) max = g;
  }
  return { min, max };
}

export function HealthDataGuard({ healthSummary }: { healthSummary: HealthSummary }) {
  const { specialCategoryTotal, standardCount, specialCategoryCount, redactionRate, facilityParity, dirFacility, spdFacility } =
    healthSummary;
  const facilityExtremes = extremes(facilityParity);
  const dirFails = dirFacility !== null && dirFacility < DIR_THRESHOLD;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-4 gap-4">
        <KPI
          label="Special-category health detections"
          value={specialCategoryTotal}
          unit="total"
          tone="warn"
          Icon={HeartPulse}
        />
        <KPI label="Redaction rate" value={redactionRate.toFixed(1)} unit="%" tone="good" Icon={ShieldCheck} />
        <KPI label="Standard-sensitivity rows" value={standardCount} unit="rows" Icon={PieChart} />
        <KPI
          label="Special-category rows"
          value={specialCategoryCount}
          unit="rows"
          tone="warn"
          Icon={PieChart}
        />
      </div>

      <Panel
        title="Why This Matters"
        sub="The stigma-aware aggregate-reporting principle behind this view — read before interpreting the numbers above"
      >
        <div className="flex items-start gap-3">
          <Info size={18} className="text-[#8A6323] shrink-0 mt-0.5" />
          <div className="text-sm text-[#14171C] space-y-2 leading-relaxed">
            <p>
              The KPI strip above shows a <strong>total</strong> count of special-category health detections, and a split by{" "}
              <strong>sensitivity class</strong> (standard vs. special-category-health) — and deliberately stops there. It does
              not, and will not, show a breakdown by specific condition, medication, or diagnosis type (e.g. "N detections were
              HIV-related this week"), even though that number technically exists in the underlying data.
            </p>
            <p>
              That is not a missing feature — it is a deliberate design decision. In the Zimbabwean context this module was
              built for, HIV status carries real, well-documented social stigma. A per-department or per-week breakdown by
              condition type, even without names attached, can be enough to identify who a detection came from once a group is
              small enough — a handful of diagnosis-code detections attributed to one small team over one short window can
              quietly point back to a specific person. That risk does not require anyone to act maliciously; it can happen just
              from ordinary dashboard use. So this view only ever aggregates upward — a total, and a coarse sensitivity-class
              split — never downward into condition-level detail.
            </p>
            <p>
              This is <strong>not</strong> a restriction on individual audit records. A compliance officer reviewing one
              specific, already-flagged session in the Audit Log view can still see exactly which entity types were detected
              in that one row — that is the legitimate, authorized, per-case review this product exists to support. The
              restriction is specifically on <em>aggregate</em> and <em>trend</em> views like this one, not on that per-entry
              detail.
            </p>
          </div>
        </div>
      </Panel>

      <Panel
        title="Facility-Type Detection Parity"
        sub="Same Disparate Impact Ratio / Statistical Parity Difference method as the Fairness Audit view, applied to facility type (e.g. rural clinic vs. urban hospital) instead of department or language, scoped to special-category health detections"
      >
        <div className="grid grid-cols-3 gap-6">
          <div className="col-span-2 h-52">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={facilityParity} layout="vertical" margin={{ left: 10 }}>
                <CartesianGrid stroke="#E1E4E8" horizontal={false} />
                <XAxis type="number" domain={[0, 12]} tick={{ fill: "#5B6270", fontSize: 11 }} unit="%" />
                <YAxis type="category" dataKey="group" tick={{ fill: "#14171C", fontSize: 12 }} width={110} />
                <Tooltip contentStyle={{ backgroundColor: "#FFFFFF", border: "1px solid #E1E4E8", fontSize: 12 }} />
                <Bar dataKey="flagRate" radius={[0, 3, 3, 0]}>
                  {facilityParity.map((entry, i) => (
                    <Cell
                      key={i}
                      fill={facilityExtremes && entry.group === facilityExtremes.min.group ? "#8A6323" : "#8A93A1"}
                    />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
          <div className="space-y-3">
            <div>
              <p className="text-[#8A93A1] text-xs">Disparate Impact Ratio</p>
              <p className="font-mono text-2xl" style={{ color: dirFails ? "#A83A3A" : "#2F7A53" }}>
                {dirFacility !== null ? dirFacility.toFixed(2) : "—"}
              </p>
              <p className="text-[10px] text-[#8A93A1]">threshold: 0.80 - {dirFails ? "FAILS, review triggered" : "pass"}</p>
            </div>
            <div>
              <p className="text-[#8A93A1] text-xs">Statistical Parity Difference</p>
              <p className="font-mono text-2xl text-[#14171C]">{spdFacility !== null ? `${spdFacility.toFixed(1)}pp` : "—"}</p>
              <p className="text-[10px] text-[#8A93A1]">target: under 5.0pp</p>
            </div>
          </div>
        </div>
        {dirFails && facilityExtremes && (
          <div className="mt-4 flex items-start gap-2 bg-[#A83A3A1A] border border-[#A83A3A55] rounded p-3">
            <AlertTriangle size={16} className="text-[#A83A3A] shrink-0 mt-0.5" />
            <p className="text-xs text-[#14171C]">
              {facilityExtremes.min.group} flagged at {facilityExtremes.min.flagRate.toFixed(1)}% vs.{" "}
              {facilityExtremes.max.flagRate.toFixed(1)}% for {facilityExtremes.max.group} - ratio {dirFacility?.toFixed(2)}{" "}
              falls below the 80% bar. Mandatory pattern-rule review opened automatically.
            </p>
          </div>
        )}
        {facilityParity.length === 0 && (
          <p className="text-xs text-[#8A93A1]">
            No facility-tagged special-category health rows yet — facility_type is an optional, caller-declared field (see
            docs/HEALTH_MODULE.md); this chart populates once at least one /api/scan call supplies it.
          </p>
        )}
      </Panel>
    </div>
  );
}

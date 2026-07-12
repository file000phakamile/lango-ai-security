"use client";

import { useEffect, useState } from "react";
import { AlertTriangle, Circle, FileText, HeartPulse, KeyRound, Radio, Scale, Shield } from "lucide-react";
import { Badge } from "./atoms";
import { CommandCenter } from "./command-center";
import { AuditLog } from "./audit-log";
import { FairnessAudit } from "./fairness-audit";
import { DriftMonitor } from "./drift-monitor";
import { PilotStatus } from "./pilot-status";
import { HealthDataGuard } from "./health-data-guard";
import { loadDashboardData, type DashboardData } from "@/lib/lango/api-client";
import type { NavItem } from "@/lib/lango/types";

const NAV: NavItem[] = [
  { key: "command", label: "Command Center", Icon: Radio },
  { key: "audit", label: "Audit Log", Icon: FileText },
  { key: "fairness", label: "Fairness Audit", Icon: Scale },
  { key: "drift", label: "Drift & Security", Icon: AlertTriangle },
  { key: "pilot", label: "Pilot & Sandbox", Icon: KeyRound },
  // Sixth view, added by the health module (Cimas Healthathon 3.0 — see
  // docs/HEALTH_MODULE.md). Appended, not inserted — the five existing
  // entries above keep their original order and keys unchanged.
  { key: "health", label: "Health Data Guard", Icon: HeartPulse },
];

export function LangoDashboard() {
  const [view, setView] = useState("command");
  const [data, setData] = useState<DashboardData | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadDashboardData().then((d) => {
      if (!cancelled) setData(d);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const activeNav = NAV.find((n) => n.key === view)!;

  if (!data) {
    return (
      <div className="min-h-screen w-full bg-[#F6F7F8] text-[#8A93A1] flex items-center justify-center font-sans text-sm">
        Loading Lango dashboard…
      </div>
    );
  }
  const log = data.log;

  return (
    <div className="min-h-screen w-full bg-[#F6F7F8] text-[#14171C] flex font-sans">
      <aside className="w-56 shrink-0 border-r border-[#E1E4E8] flex flex-col">
        <div className="px-5 py-5 border-b border-[#E1E4E8]">
          <div className="flex items-center gap-2">
            <Shield size={18} className="text-[#8A6323]" />
            <span className="font-semibold tracking-wide">LANGO</span>
          </div>
          <p className="text-[10px] text-[#8A93A1] mt-1 tracking-wide">AI DATA GUARD</p>
        </div>
        <nav className="flex-1 py-3">
          {NAV.map((n) => (
            <button
              key={n.key}
              onClick={() => setView(n.key)}
              className={`w-full flex items-center gap-3 px-5 py-2.5 text-sm text-left transition-colors ${
                view === n.key
                  ? "bg-[#F0F1F3] text-[#14171C] border-l-2 border-[#8A6323]"
                  : "text-[#5B6270] border-l-2 border-transparent hover:text-[#14171C]"
              }`}
            >
              <n.Icon size={15} />
              {n.label}
            </button>
          ))}
        </nav>
        <div className="px-5 py-4 border-t border-[#E1E4E8] text-[10px] text-[#8A93A1] leading-relaxed">
          Regulated institution demo instance.
          <br />
          No raw prompts stored.
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto">
        <header className="px-8 py-5 border-b border-[#E1E4E8] flex items-center justify-between">
          <div>
            <h1 className="text-lg font-semibold">{activeNav.label}</h1>
            <p className="text-xs text-[#8A93A1] mt-0.5">
              Pilot Institution: Regional Commercial Bank (candidate) - Credit Risk department
            </p>
          </div>
          <Badge color={data.source === "live" ? "#2F7A53" : "#8A6323"}>
            <Circle size={7} fill={data.source === "live" ? "#2F7A53" : "#8A6323"} className="mr-0.5" />
            {data.source === "live" ? "system operational" : "mock data (backend unavailable)"}
          </Badge>
        </header>
        <div className="p-8">
          {view === "command" && <CommandCenter log={log} summary={data.summary} />}
          {view === "audit" && <AuditLog log={log} />}
          {view === "fairness" && (
            <FairnessAudit
              languageParity={data.languageParity}
              departmentParity={data.departmentParity}
              dirLanguage={data.dirLanguage}
              spdLanguage={data.spdLanguage}
              dirDepartment={data.dirDepartment}
            />
          )}
          {view === "drift" && <DriftMonitor weeks={data.driftWeeks} securityEvents={data.securityEvents} />}
          {view === "pilot" && <PilotStatus />}
          {view === "health" && <HealthDataGuard healthSummary={data.healthSummary} />}
        </div>
      </main>
    </div>
  );
}

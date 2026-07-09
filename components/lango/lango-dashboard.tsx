"use client";

import { useMemo, useState } from "react";
import { AlertTriangle, Circle, FileText, KeyRound, Radio, Scale, Shield } from "lucide-react";
import { Badge } from "./atoms";
import { CommandCenter } from "./command-center";
import { AuditLog } from "./audit-log";
import { FairnessAudit } from "./fairness-audit";
import { DriftMonitor } from "./drift-monitor";
import { PilotStatus } from "./pilot-status";
import { generateAuditLog } from "@/lib/lango/mock-data";
import type { NavItem } from "@/lib/lango/types";

const NAV: NavItem[] = [
  { key: "command", label: "Command Center", Icon: Radio },
  { key: "audit", label: "Audit Log", Icon: FileText },
  { key: "fairness", label: "Fairness Audit", Icon: Scale },
  { key: "drift", label: "Drift & Security", Icon: AlertTriangle },
  { key: "pilot", label: "Pilot & Sandbox", Icon: KeyRound },
];

export function LangoDashboard() {
  const [view, setView] = useState("command");
  const log = useMemo(() => generateAuditLog(46), []);

  const activeNav = NAV.find((n) => n.key === view)!;

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
          <Badge color="#2F7A53">
            <Circle size={7} fill="#2F7A53" className="mr-0.5" />
            system operational
          </Badge>
        </header>
        <div className="p-8">
          {view === "command" && <CommandCenter log={log} />}
          {view === "audit" && <AuditLog log={log} />}
          {view === "fairness" && <FairnessAudit />}
          {view === "drift" && <DriftMonitor />}
          {view === "pilot" && <PilotStatus />}
        </div>
      </main>
    </div>
  );
}

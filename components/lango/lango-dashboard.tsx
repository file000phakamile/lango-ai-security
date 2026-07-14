"use client";

import { useEffect, useState } from "react";
import { AlertTriangle, Circle, FileDown, FileText, HeartPulse, KeyRound, Menu, Radio, Scale, Shield, SlidersHorizontal, X } from "lucide-react";
import { Badge } from "./atoms";
import { CommandCenter } from "./command-center";
import { AuditLog } from "./audit-log";
import { FairnessAudit } from "./fairness-audit";
import { DriftMonitor } from "./drift-monitor";
import { PilotStatus } from "./pilot-status";
import { HealthDataGuard } from "./health-data-guard";
import { PolicyBuilder } from "./policy-builder";
import { ComplianceExport } from "./compliance-export";
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
  // Seventh view, added by the policy builder (product-depth task, Part 1).
  // Same "append, don't reorder" convention as `health` above.
  { key: "policy", label: "Policy Builder", Icon: SlidersHorizontal },
  // Eighth view, added by compliance export (product-depth task, Part 2).
  { key: "export", label: "Compliance Export", Icon: FileDown },
];

export function LangoDashboard() {
  const [view, setView] = useState("command");
  const [data, setData] = useState<DashboardData | null>(null);
  // Mobile sidebar drawer state — irrelevant above the `md` breakpoint,
  // where the sidebar is always visible and this toggle/backdrop never
  // render at all (see the `md:hidden` / `md:flex` classes below). Below
  // `md`, the sidebar becomes a fixed-position slide-out drawer instead of
  // squeezing all main content into ~150px, which is the exact failure
  // mode docs/TESTING_LOG.md documented at 375px width.
  const [drawerOpen, setDrawerOpen] = useState(false);

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
      {/* Backdrop: only ever rendered while the drawer is open, and only
          interactive/visible below `md` (the `md:hidden` class removes it
          entirely at desktop widths regardless of `drawerOpen`, so a stale
          `true` state from a mobile->desktop resize can't leave a phantom
          overlay behind). */}
      {drawerOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/40 md:hidden"
          onClick={() => setDrawerOpen(false)}
          aria-hidden="true"
        />
      )}

      <aside
        className={`fixed inset-y-0 left-0 z-50 w-56 shrink-0 border-r border-[#E1E4E8] bg-[#F6F7F8] flex flex-col transition-transform duration-200 ease-out ${
          drawerOpen ? "translate-x-0" : "-translate-x-full"
        } md:static md:translate-x-0`}
      >
        <div className="px-5 py-5 border-b border-[#E1E4E8] flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <Shield size={18} className="text-[#8A6323]" />
              <span className="font-semibold tracking-wide">LANGO</span>
            </div>
            <p className="text-[10px] text-[#8A93A1] mt-1 tracking-wide">AI DATA GUARD</p>
          </div>
          {/* Close button only exists as a drawer below `md` — the sidebar
              has nothing to close once it's back in normal flow. */}
          <button
            className="md:hidden text-[#8A93A1] hover:text-[#14171C]"
            onClick={() => setDrawerOpen(false)}
            aria-label="Close navigation"
          >
            <X size={18} />
          </button>
        </div>
        <nav className="flex-1 py-3">
          {NAV.map((n) => (
            <button
              key={n.key}
              onClick={() => {
                setView(n.key);
                setDrawerOpen(false);
              }}
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

      <main className="flex-1 min-w-0 overflow-y-auto">
        <header className="px-4 md:px-8 py-4 md:py-5 border-b border-[#E1E4E8] flex items-center justify-between gap-3">
          <div className="flex flex-1 min-w-0 items-center gap-3">
            {/* Hamburger toggle: only exists below `md`, where the sidebar
                is a drawer rather than always visible. */}
            <button
              className="md:hidden shrink-0 text-[#5B6270] hover:text-[#14171C]"
              onClick={() => setDrawerOpen(true)}
              aria-label="Open navigation"
            >
              <Menu size={20} />
            </button>
            <div className="min-w-0">
              <h1 className="text-lg font-semibold truncate">{activeNav.label}</h1>
              <p className="text-xs text-[#8A93A1] mt-0.5 truncate">
                Pilot Institution: Regional Commercial Bank (candidate) - Credit Risk department
              </p>
            </div>
          </div>
          {/* shrink-0: the title above is the one that truncates under
              pressure (375px etc.) — this pill must keep its full text and
              shape, never get squeezed into a wrapped/broken badge. */}
          <div className="shrink-0">
            <Badge color={data.source === "live" ? "#2F7A53" : "#8A6323"}>
              <Circle size={7} fill={data.source === "live" ? "#2F7A53" : "#8A6323"} className="mr-0.5" />
              {data.source === "live" ? "system operational" : "mock data (backend unavailable)"}
            </Badge>
          </div>
        </header>
        <div className="p-4 md:p-8">
          {view === "command" && <CommandCenter log={log} summary={data.summary} />}
          {view === "audit" && <AuditLog log={log} source={data.source} />}
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
          {view === "policy" && <PolicyBuilder source={data.source} />}
          {view === "export" && <ComplianceExport source={data.source} />}
        </div>
      </main>
    </div>
  );
}

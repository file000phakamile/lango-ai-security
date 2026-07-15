"use client";

import { useEffect, useState } from "react";
import { AlertTriangle, Loader2, RefreshCw } from "lucide-react";
import { Badge, Panel } from "./atoms";
import { fetchBackendErrors } from "@/lib/lango/api-client";
import type { BackendErrorEntry } from "@/lib/lango/types";

/// Real observability (product-depth task, Part 2): a simple dashboard view
/// for recent backend errors — the internal fallback for a third-party
/// error-tracking service (see Questions.md for why a free-tier service
/// like Sentry wasn't wired in directly: it needs an account/DSN only the
/// person running this deployment can provision, which this pass had no
/// way to do). Live-only, same reasoning as every other admin view in this
/// dashboard — there is nothing real to show from mock data.
export function SystemHealth({ source }: { source: "live" | "mock" }) {
  const [errors, setErrors] = useState<BackendErrorEntry[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    if (source !== "live") return;
    let cancelled = false;
    fetchBackendErrors()
      .then((data) => {
        if (!cancelled) setErrors(data);
      })
      .catch((err) => {
        if (!cancelled) setLoadError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [source]);

  async function handleRefresh() {
    setRefreshing(true);
    setLoadError(null);
    try {
      const data = await fetchBackendErrors();
      setErrors(data);
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : String(err));
    } finally {
      setRefreshing(false);
    }
  }

  if (source !== "live") {
    return (
      <Panel title="System Health" sub="Recent backend errors and uptime status">
        <div className="flex items-start gap-2 text-sm text-[#8A6323] bg-[#8A63231A] border border-[#8A632355] rounded-md p-3">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <p>
            System Health needs the live backend — there is nothing real to show from mock data.
            Start the backend (<code className="font-mono">cargo run</code>) and reload to use it.
          </p>
        </div>
      </Panel>
    );
  }

  return (
    <div className="space-y-4">
      <Panel
        title="System Health"
        sub="Recent backend errors, so a problem can be spotted before a user reports it"
        right={
          <button
            type="button"
            onClick={handleRefresh}
            disabled={refreshing}
            className="flex items-center gap-1.5 text-xs text-[#5B6270] hover:text-[#14171C] disabled:opacity-50"
          >
            <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} /> Refresh
          </button>
        }
      >
        {loadError && (
          <p className="text-xs text-[#A83A3A] bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2 mb-3">
            Could not load recent backend errors right now. Try refreshing in a moment.
          </p>
        )}
        {!errors && !loadError && (
          <p className="text-sm text-[#8A93A1] flex items-center gap-2">
            <Loader2 size={14} className="animate-spin" /> Loading…
          </p>
        )}
        {errors && errors.length === 0 && (
          <p className="text-sm text-[#2F7A53]">No backend errors recorded. Everything looks healthy.</p>
        )}
        {errors && errors.length > 0 && (
          <div className="overflow-x-auto">
            <table className="w-full text-xs font-mono">
              <thead>
                <tr className="text-[#8A93A1] text-left border-b border-[#E1E4E8]">
                  <th className="pb-2 pr-4 font-normal">time</th>
                  <th className="pb-2 pr-4 font-normal">method</th>
                  <th className="pb-2 pr-4 font-normal">path</th>
                  <th className="pb-2 pr-4 font-normal">status</th>
                  <th className="pb-2 font-normal">message</th>
                </tr>
              </thead>
              <tbody>
                {errors.map((e) => (
                  <tr key={e.id} className="border-b border-[#E1E4E8]">
                    <td className="py-2 pr-4 text-[#5B6270] whitespace-nowrap">
                      {new Date(e.createdAt).toLocaleString()}
                    </td>
                    <td className="py-2 pr-4 text-[#14171C]">{e.method}</td>
                    <td className="py-2 pr-4 text-[#14171C]">{e.path}</td>
                    <td className="py-2 pr-4">
                      <Badge color="#A83A3A">{e.statusCode}</Badge>
                    </td>
                    <td className="py-2 text-[#5B6270] font-sans">{e.message ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <p className="text-xs text-[#8A93A1] mt-4 leading-relaxed">
          Shows the 100 most recent errors across the whole deployment.
        </p>
      </Panel>
    </div>
  );
}

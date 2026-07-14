"use client";

import { useEffect, useState } from "react";
import { AlertTriangle, Loader2, Plus, Trash2 } from "lucide-react";
import { Panel } from "./atoms";
import {
  createCustomPattern,
  deleteCustomPattern,
  fetchPolicySettings,
  updatePolicyThreshold,
} from "@/lib/lango/api-client";
import type { PolicySettings } from "@/lib/lango/types";

/// Policy builder (product-depth task, Part 1): lets a compliance_admin
/// adjust their own organisation's confidence threshold within the safe,
/// hard-coded bounds the backend returns (`minConfidenceThreshold` /
/// `maxConfidenceThreshold` — see backend/src/detection/scan.rs's
/// MIN_ORG_CONFIDENCE_THRESHOLD / MAX_ORG_CONFIDENCE_THRESHOLD), and add
/// organisation-specific structured-identifier patterns. Deliberately
/// live-only: unlike every other view in this dashboard, there is no
/// mock-data fallback here, because a fabricated threshold value would
/// misrepresent a number that actually controls what live scans do — see
/// `source` below.
export function PolicyBuilder({ source }: { source: "live" | "mock" }) {
  const [settings, setSettings] = useState<PolicySettings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [thresholdInput, setThresholdInput] = useState("");
  const [thresholdSaving, setThresholdSaving] = useState(false);
  const [thresholdError, setThresholdError] = useState<string | null>(null);
  const [thresholdSavedAt, setThresholdSavedAt] = useState<number | null>(null);

  const [labelInput, setLabelInput] = useState("");
  const [patternInput, setPatternInput] = useState("");
  const [confidenceInput, setConfidenceInput] = useState("");
  const [patternSaving, setPatternSaving] = useState(false);
  const [patternError, setPatternError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  useEffect(() => {
    if (source !== "live") return;
    let cancelled = false;
    fetchPolicySettings()
      .then((s) => {
        if (cancelled) return;
        setSettings(s);
        setThresholdInput(s.confidenceThreshold.toFixed(2));
      })
      .catch((err) => {
        if (!cancelled) setLoadError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [source]);

  if (source !== "live") {
    return (
      <Panel title="Policy Builder" sub="Confidence thresholds and organisation-specific detection patterns">
        <div className="flex items-start gap-2 text-sm text-[#8A6323] bg-[#8A63231A] border border-[#8A632355] rounded-md p-3">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <p>
            Policy Builder needs the live backend — there is no mock-data version of this view, since a
            fabricated threshold value would misrepresent a setting that actually controls live scans. Start
            the backend (<code className="font-mono">cargo run</code>) and reload to use it.
          </p>
        </div>
      </Panel>
    );
  }

  if (loadError) {
    return (
      <Panel title="Policy Builder" sub="Confidence thresholds and organisation-specific detection patterns">
        <div className="flex items-start gap-2 text-sm text-[#A83A3A] bg-[#A83A3A1A] border border-[#A83A3A55] rounded-md p-3">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <p>Could not load policy settings: {loadError}</p>
        </div>
      </Panel>
    );
  }

  if (!settings) {
    return (
      <Panel title="Policy Builder" sub="Confidence thresholds and organisation-specific detection patterns">
        <p className="text-sm text-[#8A93A1] flex items-center gap-2">
          <Loader2 size={14} className="animate-spin" /> Loading policy settings…
        </p>
      </Panel>
    );
  }

  async function saveThreshold() {
    const value = Number(thresholdInput);
    setThresholdError(null);
    if (Number.isNaN(value)) {
      setThresholdError("Enter a number.");
      return;
    }
    setThresholdSaving(true);
    try {
      const updated = await updatePolicyThreshold(value);
      setSettings(updated);
      setThresholdInput(updated.confidenceThreshold.toFixed(2));
      setThresholdSavedAt(Date.now());
    } catch (err) {
      // The backend's own rejection message (e.g. "confidence_threshold
      // must be between 0.50 and 0.95...") surfaces here verbatim — this is
      // the real, server-enforced bound speaking, not client-side validation
      // text.
      setThresholdError(err instanceof Error ? err.message : String(err));
    } finally {
      setThresholdSaving(false);
    }
  }

  async function addPattern() {
    setPatternError(null);
    if (!labelInput.trim() || !patternInput.trim()) {
      setPatternError("Both a label and a pattern are required.");
      return;
    }
    const confidence = confidenceInput.trim() === "" ? undefined : Number(confidenceInput);
    if (confidence !== undefined && Number.isNaN(confidence)) {
      setPatternError("Confidence must be a number.");
      return;
    }
    setPatternSaving(true);
    try {
      const updated = await createCustomPattern(labelInput.trim(), patternInput.trim(), confidence);
      setSettings(updated);
      setLabelInput("");
      setPatternInput("");
      setConfidenceInput("");
    } catch (err) {
      setPatternError(err instanceof Error ? err.message : String(err));
    } finally {
      setPatternSaving(false);
    }
  }

  async function removePattern(id: string) {
    setDeletingId(id);
    try {
      const updated = await deleteCustomPattern(id);
      setSettings(updated);
    } catch (err) {
      setPatternError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div className="space-y-4">
      <Panel
        title="Confidence Threshold"
        sub="How confident a match must be before it's redacted and forwarded rather than blocked - scoped to your organisation only"
      >
        <div className="flex flex-col sm:flex-row sm:items-end gap-4">
          <div>
            <label htmlFor="policy-threshold-input" className="block text-xs text-[#8A93A1] mb-1">
              Current threshold (safe range: {settings.minConfidenceThreshold.toFixed(2)} -{" "}
              {settings.maxConfidenceThreshold.toFixed(2)})
            </label>
            <input
              id="policy-threshold-input"
              type="number"
              step="0.01"
              min={settings.minConfidenceThreshold}
              max={settings.maxConfidenceThreshold}
              value={thresholdInput}
              onChange={(e) => setThresholdInput(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-1.5 font-mono w-32"
            />
          </div>
          <button
            type="button"
            onClick={saveThreshold}
            disabled={thresholdSaving}
            className="bg-[#14171C] text-white text-sm rounded px-4 py-1.5 hover:bg-[#2A2E36] disabled:opacity-50 w-fit"
          >
            {thresholdSaving ? "Saving…" : "Save threshold"}
          </button>
          {thresholdSavedAt && !thresholdError && (
            <span className="text-xs text-[#2F7A53]">Saved.</span>
          )}
        </div>
        {thresholdError && (
          <p className="text-xs text-[#A83A3A] mt-3 bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2">
            {thresholdError}
          </p>
        )}
        <p className="text-xs text-[#8A93A1] mt-3 leading-relaxed">
          {`This bound cannot be widened past ${settings.minConfidenceThreshold.toFixed(2)}-${settings.maxConfidenceThreshold.toFixed(2)} from this screen or via the API - it exists so a misconfiguration can't disable the fail-closed guarantee. The special-category health data hard rule (low-confidence health-related matches always block, never get the lenient review path) is not configurable at all, by anyone, and is not shown here as a setting because it isn't one.`}
        </p>
      </Panel>

      <Panel
        title="Organisation-Specific Patterns"
        sub="Structured identifier formats unique to your organisation (e.g. your own account-number format) - matched alongside the built-in detectors, applied only to your organisation's scans"
      >
        <div className="space-y-2 mb-4">
          {settings.customPatterns.length === 0 && (
            <p className="text-xs text-[#8A93A1]">No custom patterns yet.</p>
          )}
          {settings.customPatterns.map((p) => (
            <div
              key={p.id}
              className="flex items-center justify-between gap-3 border border-[#E1E4E8] rounded-md px-3 py-2"
            >
              <div className="min-w-0">
                <p className="text-sm font-mono text-[#14171C] truncate">{p.entityLabel}</p>
                <p className="text-xs font-mono text-[#5B6270] truncate">{p.pattern}</p>
                <p className="text-[10px] text-[#8A93A1] mt-0.5">
                  confidence {p.confidence.toFixed(2)} · added {new Date(p.createdAt).toLocaleDateString()}
                </p>
              </div>
              <button
                type="button"
                onClick={() => removePattern(p.id)}
                disabled={deletingId === p.id}
                aria-label={`Delete pattern ${p.entityLabel}`}
                className="shrink-0 text-[#8A93A1] hover:text-[#A83A3A] disabled:opacity-50"
              >
                <Trash2 size={15} />
              </button>
            </div>
          ))}
        </div>

        <div className="border-t border-[#E1E4E8] pt-4">
          <p className="text-xs text-[#8A93A1] mb-2">Add a pattern</p>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
            <input
              type="text"
              placeholder="label (e.g. acme_account_format)"
              value={labelInput}
              onChange={(e) => setLabelInput(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-xs rounded px-3 py-1.5 font-mono"
            />
            <input
              type="text"
              placeholder="regex pattern (e.g. ACME-\d{8})"
              value={patternInput}
              onChange={(e) => setPatternInput(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-xs rounded px-3 py-1.5 font-mono"
            />
            <input
              type="text"
              placeholder="confidence (optional, default 0.80)"
              value={confidenceInput}
              onChange={(e) => setConfidenceInput(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-xs rounded px-3 py-1.5 font-mono"
            />
          </div>
          <button
            type="button"
            onClick={addPattern}
            disabled={patternSaving}
            className="mt-3 flex items-center gap-1.5 bg-[#14171C] text-white text-sm rounded px-4 py-1.5 hover:bg-[#2A2E36] disabled:opacity-50 w-fit"
          >
            <Plus size={14} /> {patternSaving ? "Adding…" : "Add pattern"}
          </button>
          {patternError && (
            <p className="text-xs text-[#A83A3A] mt-3 bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2">
              {patternError}
            </p>
          )}
        </div>
      </Panel>
    </div>
  );
}

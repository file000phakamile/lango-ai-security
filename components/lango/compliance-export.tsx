"use client";

import { useState } from "react";
import { FileDown, Loader2 } from "lucide-react";
import { Panel, UnavailableNotice } from "./atoms";
import {
  downloadComplianceExport,
  downloadLabelledDataset,
} from "@/lib/lango/api-client";

function isoDaysAgo(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

/// Compliance export (product-depth task, Part 2): a one-click, date-ranged
/// CSV or PDF export of the audit log, fairness metrics, and drift history
/// for the caller's own organisation. Live-only, same reasoning as
/// PolicyBuilder — there's nothing meaningful to export from mock data.
export function ComplianceExport({ source }: { source: "live" | "mock" }) {
  const [start, setStart] = useState(isoDaysAgo(90));
  const [end, setEnd] = useState(isoDaysAgo(0));
  const [downloading, setDownloading] = useState<"csv" | "pdf" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastDownloaded, setLastDownloaded] = useState<"csv" | "pdf" | null>(
    null,
  );

  if (source !== "live") {
    return (
      <Panel
        title="Compliance Export"
        sub="Audit-ready CSV or PDF export for a selected date range"
      >
        <UnavailableNotice />
      </Panel>
    );
  }

  async function handleDownload(format: "csv" | "pdf") {
    setError(null);
    setLastDownloaded(null);
    if (start > end) {
      setError("Start date must not be after end date.");
      return;
    }
    setDownloading(format);
    try {
      await downloadComplianceExport(start, end, format);
      setLastDownloaded(format);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDownloading(null);
    }
  }

  return (
    <div className="space-y-4">
      <Panel
        title="Compliance Export"
        sub="One-click CSV or PDF export of the audit log, fairness metrics, and drift history for a selected date range - ready to hand to an external auditor or regulator"
      >
        <div className="flex flex-col sm:flex-row sm:items-end gap-4 flex-wrap">
          <div>
            <label
              htmlFor="export-start"
              className="block text-xs text-[#8A93A1] mb-1"
            >
              Start date
            </label>
            <input
              id="export-start"
              type="date"
              value={start}
              max={end}
              onChange={(e) => setStart(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-1.5 font-mono"
            />
          </div>
          <div>
            <label
              htmlFor="export-end"
              className="block text-xs text-[#8A93A1] mb-1"
            >
              End date
            </label>
            <input
              id="export-end"
              type="date"
              value={end}
              min={start}
              onChange={(e) => setEnd(e.target.value)}
              className="bg-[#F6F7F8] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-3 py-1.5 font-mono"
            />
          </div>
          <button
            type="button"
            onClick={() => handleDownload("csv")}
            disabled={downloading !== null}
            className="flex items-center gap-1.5 bg-[#14171C] text-white text-sm rounded px-4 py-1.5 hover:bg-[#2A2E36] disabled:opacity-50 w-fit"
          >
            <FileDown size={14} />{" "}
            {downloading === "csv" ? "Preparing…" : "Download CSV"}
          </button>
          <button
            type="button"
            onClick={() => handleDownload("pdf")}
            disabled={downloading !== null}
            className="flex items-center gap-1.5 bg-[#FFFFFF] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-4 py-1.5 hover:bg-[#F0F1F3] disabled:opacity-50 w-fit"
          >
            <FileDown size={14} />{" "}
            {downloading === "pdf" ? "Preparing…" : "Download PDF"}
          </button>
        </div>

        {downloading && (
          <p className="text-xs text-[#5B6270] mt-3 flex items-center gap-2">
            <Loader2 size={12} className="animate-spin" /> Generating your{" "}
            {downloading.toUpperCase()} export…
          </p>
        )}
        {lastDownloaded && !error && (
          <p className="text-xs text-[#2F7A53] mt-3">
            {lastDownloaded.toUpperCase()} export downloaded.
          </p>
        )}
        {error && (
          <p className="text-xs text-[#A83A3A] mt-3 bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2">
            {error}
          </p>
        )}

        <p className="text-xs text-[#8A93A1] mt-4 leading-relaxed">
          The CSV contains the complete, unabridged dataset for the selected
          range. The PDF is a readable, printable summary of the same three
          sections (audit log, fairness metrics, drift history), capped at the
          500 most recent audit log rows in range - use the CSV for the complete
          record.
        </p>
      </Panel>

      <LabelledDatasetExport />
    </div>
  );
}

/// Active learning loop (product-depth task, Part 3): a simple export of
/// every human confirm/overturn judgment recorded so far via the Audit
/// Log's row-expand (see `components/lango/audit-log.tsx`'s `ReviewSection`).
/// Deliberately no date range (unlike ComplianceExport above) - every
/// labelled example an organisation has ever produced is training/rule-
/// tuning signal, so this exports everything. This component ONLY exports
/// already-recorded human decisions - nothing here or anywhere else in this
/// codebase retrains or fine-tunes anything automatically from this data.
function LabelledDatasetExport() {
  const [downloading, setDownloading] = useState<"csv" | "jsonl" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastDownloaded, setLastDownloaded] = useState<"csv" | "jsonl" | null>(
    null,
  );

  async function handleDownload(format: "csv" | "jsonl") {
    setError(null);
    setLastDownloaded(null);
    setDownloading(format);
    try {
      await downloadLabelledDataset(format);
      setLastDownloaded(format);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDownloading(null);
    }
  }

  return (
    <Panel
      title="Labelled Dataset (Active Learning)"
      sub="Every human confirm/overturn judgment recorded on a flagged low-confidence row, exported for future rule-tuning - this only captures the signal, nothing here retrains or fine-tunes anything automatically"
    >
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          onClick={() => handleDownload("csv")}
          disabled={downloading !== null}
          className="flex items-center gap-1.5 bg-[#14171C] text-white text-sm rounded px-4 py-1.5 hover:bg-[#2A2E36] disabled:opacity-50 w-fit"
        >
          <FileDown size={14} />{" "}
          {downloading === "csv" ? "Preparing…" : "Download CSV"}
        </button>
        <button
          type="button"
          onClick={() => handleDownload("jsonl")}
          disabled={downloading !== null}
          className="flex items-center gap-1.5 bg-[#FFFFFF] border border-[#E1E4E8] text-[#14171C] text-sm rounded px-4 py-1.5 hover:bg-[#F0F1F3] disabled:opacity-50 w-fit"
        >
          <FileDown size={14} />{" "}
          {downloading === "jsonl" ? "Preparing…" : "Download JSONL"}
        </button>
      </div>
      {lastDownloaded && !error && (
        <p className="text-xs text-[#2F7A53] mt-3">
          {lastDownloaded.toUpperCase()} export downloaded.
        </p>
      )}
      {error && (
        <p className="text-xs text-[#A83A3A] mt-3 bg-[#A83A3A1A] border border-[#A83A3A55] rounded px-3 py-2">
          {error}
        </p>
      )}
      <p className="text-xs text-[#8A93A1] mt-4 leading-relaxed">
        JSONL (one JSON object per line) is the shape most rule-tuning tooling
        ingests directly; the CSV covers the same data for a spreadsheet.
        Confirm/overturn a flagged row from the Audit Log view to add to this
        dataset.
      </p>
    </Panel>
  );
}

import { BookOpen, Building2, Puzzle } from "lucide-react";
import { Panel } from "./atoms";

/// The dashboard's own "how do I use this" page — a real in-app view, not a
/// link out to a file the reader would have to leave the product to open.
/// Content is kept in sync with HOW_TO_USE.md at the project root by hand
/// (same headings, same facts, same order) rather than by a single generated
/// source, since adding a markdown-rendering dependency for one page was
/// judged disproportionate — see Questions.md for that call. Visible
/// regardless of role, same as every other dashboard view today (this
/// dashboard has no per-role frontend gating yet — see Questions.md).
export function Help() {
  return (
    <div className="space-y-5">
      <Panel title="The two parts of Lango" sub="Two separate tools for two different people">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
          <div className="flex gap-3">
            <Puzzle size={18} className="text-[#8A6323] shrink-0 mt-0.5" />
            <div>
              <p className="font-semibold text-[#14171C]">The browser extension</p>
              <p className="text-[#5B6270] mt-1 leading-relaxed">
                What a frontline employee installs and uses day-to-day while chatting
                with ChatGPT, Claude, Gemini, and similar tools. Scans a prompt before
                it's sent, and scans the AI's reply after it arrives.
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <Building2 size={18} className="text-[#8A6323] shrink-0 mt-0.5" />
            <div>
              <p className="font-semibold text-[#14171C]">This dashboard</p>
              <p className="text-[#5B6270] mt-1 leading-relaxed">
                What a compliance or IT officer uses afterward, to review what the
                extension has been doing: what was redacted, what was blocked, what's
                flagged for review.
              </p>
            </div>
          </div>
        </div>
        <p className="text-xs text-[#8A93A1] mt-4 leading-relaxed">
          This is a v0.1 demo, not a finished commercial product — real, working code,
          not a mockup, but without a formal security audit yet. There is no
          self-service signup: this demo runs on one shared account,{" "}
          <code className="font-mono">compliance@lango.demo</code> /{" "}
          <code className="font-mono">LangoDemo123!</code> — already public, and it
          only ever protects synthetic demo data.
        </p>
      </Panel>

      <Panel title="Using the extension" sub="Install, log in, and use it on a supported site">
        <ol className="text-sm text-[#14171C] space-y-2 list-decimal list-inside">
          <li>
            <strong>Install it</strong> (Chrome or Edge): go to{" "}
            <code className="font-mono text-xs">chrome://extensions</code>, turn on
            Developer mode, click Load unpacked, and select the{" "}
            <code className="font-mono text-xs">extension/</code> folder.
          </li>
          <li>
            <strong>Find the icon</strong>: click the puzzle-piece icon near your
            address bar, find Lango, and pin it.
          </li>
          <li>
            <strong>Log in</strong> with the demo credentials above. A green dot means
            you're connected.
          </li>
          <li>
            <strong>Use it</strong>: type a prompt on a supported site and press
            Enter or click Send as normal — Lango intercepts it first, usually in
            well under a second.
          </li>
        </ol>
      </Panel>

      <Panel title="What each banner means" sub="Color is always paired with a plain-language message">
        <div className="space-y-3 text-sm">
          <div className="flex items-start gap-3">
            <span className="w-3 h-3 rounded-full bg-[#2F7A53] mt-1 shrink-0" />
            <p><strong className="text-[#14171C]">Green</strong> — nothing sensitive found, your prompt was sent unchanged. Nothing to do.</p>
          </div>
          <div className="flex items-start gap-3">
            <span className="w-3 h-3 rounded-full bg-[#8A6323] mt-1 shrink-0" />
            <p><strong className="text-[#14171C]">Gold</strong> — a sensitive entity was redacted before sending. The redacted version was sent, not your original text.</p>
          </div>
          <div className="flex items-start gap-3">
            <span className="w-3 h-3 rounded-full bg-[#C2660C] mt-1 shrink-0" />
            <p><strong className="text-[#14171C]">Amber</strong> — either a low-confidence name match was redacted and flagged for later review, or the AI's reply may contain something sensitive. The reply itself is always shown to you in full and unchanged; review it yourself before relying on it.</p>
          </div>
          <div className="flex items-start gap-3">
            <span className="w-3 h-3 rounded-full bg-[#A83A3A] mt-1 shrink-0" />
            <p><strong className="text-[#14171C]">Red</strong> — blocked. Nothing was sent. Edit your prompt and try again, or wait a few seconds if the backend was simply unreachable.</p>
          </div>
        </div>
        <p className="text-xs text-[#8A93A1] mt-4 leading-relaxed">
          Response scanning (checking the AI's reply) currently covers chatgpt.com,
          claude.ai, and gemini.google.com only, and commonly takes under 10 seconds —
          a staged loading indicator shows nothing at first, then a calm spinner, then
          a short status message if it runs past a few seconds.
        </p>
      </Panel>

      <Panel title="What each dashboard view shows" sub="A one-line summary of every sidebar item">
        <dl className="text-sm space-y-2.5">
          {[
            ["Command Center", "A live overview — sessions scanned, blocked/redacted counts, average risk score, active alerts, and a recent-events feed. Updates automatically every 15 seconds."],
            ["Audit Log", "The full, filterable record of every scan: who, when, what was detected, the decision made, and why. A flagged low-confidence row can be confirmed or overturned directly here."],
            ["Fairness Audit", "Compares how often prompts get flagged across languages and departments, so a systematic bias doesn't go unnoticed."],
            ["Drift & Security", "Tracks whether detection accuracy is drifting over time, plus a feed of security-relevant events."],
            ["Pilot & Sandbox", "The current pilot's scope, rollout checklist, and success metrics."],
            ["Health Data Guard", "The same monitoring scoped to health-related detections — deliberately only totals and coarse splits, never a per-condition breakdown."],
            ["Policy Builder", "Lets a compliance admin adjust detection sensitivity within safe bounds and add organisation-specific patterns. Health-related detections always follow the strictest rule regardless of this setting — that one isn't configurable by anyone."],
            ["Compliance Export", "One-click CSV/PDF export of the audit log, fairness metrics, and drift history, ready to hand to an auditor."],
            ["System Health", "A simple list of recent backend errors, so an operator can spot a problem without a separate monitoring tool."],
          ].map(([title, desc]) => (
            <div key={title} className="flex flex-col sm:flex-row sm:gap-3">
              <dt className="font-semibold text-[#14171C] sm:w-40 shrink-0">{title}</dt>
              <dd className="text-[#5B6270]">{desc}</dd>
            </div>
          ))}
        </dl>
      </Panel>

      <Panel title="Known limitations that actually matter" sub="Stated plainly, not buried in fine print">
        <ul className="text-sm text-[#14171C] space-y-3 list-disc list-inside">
          <li>
            <strong>Which sites are actually verified, not just implemented.</strong>{" "}
            ChatGPT's prompt scanning and Gemini's prompt and response scanning have
            both been driven against real, live sessions and confirmed working.
            Claude, DeepSeek, and Copilot's consumer web chat are implemented using a
            best-effort guess at each site's structure but not yet confirmed against a
            live page.
          </li>
          <li>
            <strong>The backend can take up to a minute to wake up.</strong> It runs
            on a free hosting tier that spins down after ~15 minutes idle. The first
            request afterward can take 30-60 seconds. This is normal.
          </li>
          <li>
            <strong>Mobile and small screens work</strong> — tested down to 375px
            width with no horizontal overflow.
          </li>
          <li>
            <strong>Response scanning is a genuinely harder problem than prompt
            scanning.</strong> Lango approximates "the reply is finished" by waiting
            for the page to stop changing — a measured, evidence-based heuristic, not
            a guarantee.
          </li>
          <li>
            <strong>One shared demo account.</strong> Every action in this demo is
            logged under one seeded user — a real deployment would give every
            employee their own login.
          </li>
        </ul>
      </Panel>

      <p className="text-xs text-[#8A93A1] flex items-center gap-1.5">
        <BookOpen size={12} /> This page mirrors HOW_TO_USE.md at the project root —
        open that file directly if you're browsing the repository instead of the
        live app.
      </p>
    </div>
  );
}

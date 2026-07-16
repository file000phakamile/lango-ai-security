import { AlertTriangle, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

export function Panel({
  title,
  sub,
  right,
  children,
  className = "",
}: {
  title?: string;
  sub?: string;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`bg-[#FFFFFF] border border-[#E1E4E8] rounded-md ${className}`}>
      {(title || right) && (
        <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-2 px-4 sm:px-5 pt-4 pb-3 border-b border-[#E1E4E8]">
          <div>
            {title && <h3 className="text-[#14171C] text-sm font-semibold tracking-wide">{title}</h3>}
            {sub && <p className="text-[#5B6270] text-xs mt-1">{sub}</p>}
          </div>
          {right}
        </div>
      )}
      <div className="p-4 sm:p-5">{children}</div>
    </div>
  );
}

// Design pass, Step 5: counts up from 0 to the real value on first mount
// rather than appearing instantly — a KPI tile refreshing mid-session
// (e.g. from polling) does NOT retrigger this, only the initial reveal
// does, since re-animating on every live update would be distracting
// motion competing with the "new activity" signal itself, not supporting
// it. Respects prefers-reduced-motion by skipping straight to the final
// value — a decorative count-up is exactly the kind of motion that
// preference exists to suppress.
function useCountUp(target: number, durationMs = 700) {
  const [display, setDisplay] = useState(0);
  const hasAnimated = useRef(false);

  useEffect(() => {
    if (hasAnimated.current) {
      setDisplay(target);
      return;
    }
    hasAnimated.current = true;

    const reduceMotion =
      typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || target === 0) {
      setDisplay(target);
      return;
    }

    let raf: number;
    const start = performance.now();
    function tick(now: number) {
      const progress = Math.min((now - start) / durationMs, 1);
      // ease-out cubic — starts fast, settles gently, rather than a linear
      // ramp that feels mechanical for a number this small.
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplay(target * eased);
      if (progress < 1) raf = requestAnimationFrame(tick);
    }
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentionally only re-runs when `target` changes identity on first mount
  }, [target, durationMs]);

  return display;
}

export function KPI({
  label,
  value,
  unit,
  tone = "neutral",
  Icon,
}: {
  label: string;
  value: string | number;
  unit?: string;
  tone?: "neutral" | "danger" | "warn" | "good";
  Icon?: LucideIcon;
}) {
  const toneColor =
    tone === "danger" ? "#A83A3A" : tone === "warn" ? "#8A6323" : tone === "good" ? "#2F7A53" : "#14171C";

  // Numeric values (session counts, alert counts) count up as whole
  // numbers; a decimal string like avgRisk's "0.82" counts up preserving
  // its own decimal precision instead of animating through integers only.
  const numericTarget = typeof value === "number" ? value : parseFloat(value);
  const decimals = typeof value === "string" && value.includes(".") ? value.split(".")[1].length : 0;
  const animated = useCountUp(Number.isFinite(numericTarget) ? numericTarget : 0);
  const displayValue = Number.isFinite(numericTarget) ? animated.toFixed(decimals) : value;

  return (
    <div className="bg-[#FFFFFF] border border-[#E1E4E8] rounded-md p-4 flex items-start justify-between">
      <div>
        <p className="text-[#5B6270] text-xs uppercase tracking-wider">{label}</p>
        <p className="mt-2 text-2xl font-mono font-semibold" style={{ color: toneColor }}>
          {displayValue}
          <span className="text-sm text-[#5B6270] ml-1 font-sans">{unit}</span>
        </p>
      </div>
      {Icon && <Icon size={18} className="text-[#8A93A1] mt-1" />}
    </div>
  );
}

// Design pass, Step 5: a real skeleton matching each view's actual layout
// (a row of blocks, a table-shaped grid, etc.) rather than a generic
// spinner or blank screen while data loads — the specific gap the given
// design research named. `barCount`/`rowHeight` let callers shape it
// roughly like the content it's standing in for without needing a bespoke
// skeleton per view.
export function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`animate-pulse rounded-md bg-[#E1E4E8] ${className}`} />;
}

export function DashboardSkeleton() {
  return (
    <div className="space-y-5" aria-hidden="true">
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="bg-[#FFFFFF] border border-[#E1E4E8] rounded-md p-4 space-y-3">
            <Skeleton className="h-3 w-24" />
            <Skeleton className="h-7 w-16" />
          </div>
        ))}
      </div>
      <div className="bg-[#FFFFFF] border border-[#E1E4E8] rounded-md p-5 space-y-4">
        <Skeleton className="h-4 w-40" />
        <div className="space-y-2">
          {[0, 1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </div>
    </div>
  );
}

export function Badge({
  color,
  children,
  title,
}: {
  color: string;
  children: ReactNode;
  // Optional native browser tooltip — used for a short "why" detail that
  // doesn't need to sit permanently in the badge's own visible text (e.g.
  // the sample-data badge's "can take up to a minute" explanation). A
  // second visible line was considered instead and rejected for this
  // specific badge: it's a compact header pill next to the page title, not
  // a panel, and forcing a two-line badge into that space would be a real
  // layout change, not just a wording one — see Questions.md.
  title?: string;
}) {
  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono border"
      style={{ color, borderColor: `${color}55`, backgroundColor: `${color}1A` }}
      title={title}
    >
      {children}
    </span>
  );
}

// Wording pass: the identical "we couldn't load this right now" state used to
// be written out separately, slightly differently worded each time, in
// System Health, Policy Builder, and Compliance Export — all three genuinely
// mean the same thing (no live connection to read real data from right now,
// most likely a normal, temporary startup delay), so one shared component
// says it once, consistently, rather than three near-duplicate strings
// drifting apart over time. Deliberately never mentions mock/sample data —
// none of these three views ever show mock data at all (a fabricated number
// here would misrepresent something that actually controls behavior or
// compliance evidence), so there is nothing to label as sample data; the
// honest thing to say is simply that nothing loaded.
export function UnavailableNotice() {
  return (
    <div className="flex items-start gap-2 text-sm text-[#8A6323] bg-[#8A63231A] border border-[#8A632355] rounded-md p-3">
      <AlertTriangle size={16} className="mt-0.5 shrink-0" />
      <p>
        We couldn&apos;t load this just now. If nothing has been used recently, the system may
        still be starting up — this can take up to a minute. Please try again shortly.
      </p>
    </div>
  );
}

import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

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
  return (
    <div className="bg-[#FFFFFF] border border-[#E1E4E8] rounded-md p-4 flex items-start justify-between">
      <div>
        <p className="text-[#5B6270] text-xs uppercase tracking-wider">{label}</p>
        <p className="mt-2 text-2xl font-mono font-semibold" style={{ color: toneColor }}>
          {value}
          <span className="text-sm text-[#5B6270] ml-1 font-sans">{unit}</span>
        </p>
      </div>
      {Icon && <Icon size={18} className="text-[#8A93A1] mt-1" />}
    </div>
  );
}

export function Badge({ color, children }: { color: string; children: ReactNode }) {
  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-mono border"
      style={{ color, borderColor: `${color}55`, backgroundColor: `${color}1A` }}
    >
      {children}
    </span>
  );
}

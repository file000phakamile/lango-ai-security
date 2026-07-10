import { Ban, CheckCircle2, Flag, ScanEye } from "lucide-react";
import type { Decision, DecisionBadgeInfo } from "@/lib/lango/types";

export function decisionBadge(decision: Decision): DecisionBadgeInfo {
  switch (decision) {
    case "redacted_and_forwarded":
      return { label: "redacted_and_forwarded", color: "#8A6323", Icon: ScanEye };
    case "blocked_low_confidence":
      return { label: "blocked_low_confidence", color: "#A83A3A", Icon: Ban };
    case "redacted_low_confidence_review":
      // Distinct from both redacted_and_forwarded's gold (#8A6323) and
      // blocked_low_confidence's red (#A83A3A) — an amber/orange reads as
      // "needs attention but isn't blocked," plus a distinct Flag icon
      // (rather than reusing ScanEye) so it's identifiable at a glance even
      // without color, e.g. for colorblind users.
      return { label: "redacted_low_confidence_review", color: "#C2660C", Icon: Flag };
    default:
      return { label: "cleared_no_entities", color: "#2F7A53", Icon: CheckCircle2 };
  }
}

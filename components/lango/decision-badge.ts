import { Ban, CheckCircle2, ScanEye } from "lucide-react";
import type { Decision, DecisionBadgeInfo } from "@/lib/lango/types";

export function decisionBadge(decision: Decision): DecisionBadgeInfo {
  switch (decision) {
    case "redacted_and_forwarded":
      return { label: "redacted_and_forwarded", color: "#8A6323", Icon: ScanEye };
    case "blocked_low_confidence":
      return { label: "blocked_low_confidence", color: "#A83A3A", Icon: Ban };
    default:
      return { label: "cleared_no_entities", color: "#2F7A53", Icon: CheckCircle2 };
  }
}

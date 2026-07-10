// content/ui-banner.js — small, unobtrusive status banner shared by every
// site adapter. Colors match the dashboard's decision language exactly
// (components/lango/decision-badge.ts): gold #8A6323 for redacted, red
// #A83A3A for blocked, green #2F7A53 for cleared, amber/orange #C2660C for
// a low-confidence-but-forwarded match flagged for review (distinct from
// both the redacted gold and the blocked red — see decision-badge.ts's own
// comment on why). Loaded first in manifest.json so later content scripts
// (any site adapter) can call showBanner() directly — MV3 content scripts
// listed together in one content_scripts entry share the same
// isolated-world global scope, in file-list order.

const LANGO_BANNER_COLORS = {
  cleared: "#2F7A53",
  redacted: "#8A6323",
  reviewFlagged: "#C2660C",
  blocked: "#A83A3A",
  neutral: "#5B6270",
};

function showBanner(message, kind, opts) {
  opts = opts || {};
  const existing = document.getElementById("lango-banner");
  if (existing) existing.remove();

  const bg = LANGO_BANNER_COLORS[kind] || LANGO_BANNER_COLORS.neutral;
  const el = document.createElement("div");
  el.id = "lango-banner";
  el.textContent = message;
  Object.assign(el.style, {
    position: "fixed",
    bottom: "100px",
    left: "50%",
    transform: "translateX(-50%)",
    background: bg,
    color: "#FFFFFF",
    padding: "10px 18px",
    borderRadius: "8px",
    fontSize: "13px",
    fontFamily: "system-ui, -apple-system, sans-serif",
    zIndex: 2147483647,
    boxShadow: "0 2px 10px rgba(0,0,0,0.3)",
    maxWidth: "520px",
    textAlign: "center",
    lineHeight: "1.4",
    // Not pointer-events: none — a blocked/error banner should stay
    // readable and selectable, not fight with the page underneath it.
  });
  document.body.appendChild(el);

  const autoDismiss = opts.autoDismiss !== false;
  if (autoDismiss) {
    setTimeout(() => {
      if (el.parentNode) el.remove();
    }, opts.duration || 4000);
  }
  return el;
}

// content/ui-banner.js — status banner + staged loading indicator, shared by
// every site adapter. Colors match the dashboard's decision language exactly
// (components/lango/decision-badge.ts): gold #8A6323 for redacted, red
// #A83A3A for blocked, green #2F7A53 for cleared, amber/orange #C2660C for
// a low-confidence-but-forwarded match flagged for review (distinct from
// both the redacted gold and the blocked red — see decision-badge.ts's own
// comment on why). Loaded first in manifest.json so later content scripts
// (any site adapter) can call showBanner()/startScanIndicator() directly —
// MV3 content scripts listed together in one content_scripts entry share
// the same isolated-world global scope, in file-list order.
//
// Design pass, Step 5: rebuilt from a single static div into a staged
// system, per the design direction given in Step 4 (Questions.md item 35):
//   - Under ~1s: nothing renders at all — a spinner for a sub-second wait
//     reads as noise, not information.
//   - ~1s-3s: a calm, simple indeterminate indicator.
//   - Past ~3s (in practice, mostly response scanning — see
//     content/response-scanner.js): honest, rotating status phrases instead
//     of one static label sitting unchanged on screen.
// Both `showBanner()` (a terminal result — cleared/redacted/blocked/flagged)
// and the staged loading indicator share the SAME element id
// ("lango-banner"), so the existing "remove whatever's there first" logic
// below is all that's needed to keep the single-banner-at-a-time invariant —
// a loading indicator and a terminal banner can never coexist, and starting
// a new scan always clears whatever the previous one left behind.

const LANGO_BANNER_COLORS = {
  cleared: "#2F7A53",
  redacted: "#8A6323",
  reviewFlagged: "#C2660C",
  blocked: "#A83A3A",
  neutral: "#5B6270",
};

// Small inline SVGs (no external asset loading — nothing new for the
// manifest to declare, nothing that can 404) extending the dashboard's
// existing "never color alone, always paired with an icon and a text label"
// principle (decision-badge.tsx) into the one surface that didn't have it
// yet — the extension's banners never had an icon language of their own to
// preserve (see Questions.md item 35).
const LANGO_BANNER_ICONS = {
  cleared:
    '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>',
  redacted:
    '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z"/></svg>',
  reviewFlagged:
    '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"/></svg>',
  blocked:
    '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="m15 9-6 6M9 9l6 6"/></svg>',
  neutral:
    '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/></svg>',
};

let stylesInjected = false;
function ensureStyles() {
  if (stylesInjected) return;
  stylesInjected = true;
  const style = document.createElement("style");
  style.id = "lango-banner-styles";
  style.textContent = `
    @keyframes lango-spin { to { transform: rotate(360deg); } }
    @keyframes lango-fade-in { from { opacity: 0; transform: translate(-50%, 4px); } to { opacity: 1; transform: translate(-50%, 0); } }
    #lango-banner.lango-exiting { opacity: 0; transform: translate(-50%, 4px); }
    #lango-banner .lango-spinner {
      width: 12px; height: 12px; border-radius: 50%;
      border: 2px solid rgba(255,255,255,0.35); border-top-color: #FFFFFF;
      animation: lango-spin 0.8s linear infinite;
      flex-shrink: 0;
    }
    @media (prefers-reduced-motion: reduce) {
      #lango-banner { animation: none !important; transition: none !important; }
      #lango-banner.lango-exiting { opacity: 1; transform: translate(-50%, 0); }
      #lango-banner .lango-spinner { animation: none !important; border-top-color: rgba(255,255,255,0.35); }
    }
  `;
  document.head.appendChild(style);
}

function prefersReducedMotion() {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function removeExisting() {
  const existing = document.getElementById("lango-banner");
  if (existing) existing.remove();
}

/**
 * Terminal result banner — a decision is known (cleared, redacted, flagged,
 * blocked), shown once, not staged. Also used internally by the loading
 * indicator to render its own (non-terminal) states, sharing one element id
 * so only one banner can ever exist at a time.
 */
function showBanner(message, kind, opts) {
  opts = opts || {};
  ensureStyles();
  removeExisting();

  const bg = LANGO_BANNER_COLORS[kind] || LANGO_BANNER_COLORS.neutral;
  const icon = LANGO_BANNER_ICONS[kind] || LANGO_BANNER_ICONS.neutral;
  const reduceMotion = prefersReducedMotion();

  const el = document.createElement("div");
  el.id = "lango-banner";
  // Design pass, Step 5: a live region so a screen-reader user is actually
  // told a scan result appeared — this extension had no accessibility
  // treatment for its banners before this. "assertive" for a blocked
  // outcome specifically (the one case where interrupting matters — nothing
  // was sent), "polite" for everything else so it doesn't talk over
  // whatever the user is already doing.
  el.setAttribute("role", "status");
  el.setAttribute("aria-live", kind === "blocked" ? "assertive" : "polite");
  el.innerHTML = `<span class="lango-banner-icon" style="display:flex;flex-shrink:0" aria-hidden="true">${icon}</span><span>${escapeHtml(message)}</span>`;
  Object.assign(el.style, {
    position: "fixed",
    bottom: "100px",
    left: "50%",
    transform: "translateX(-50%)",
    display: "flex",
    alignItems: "center",
    gap: "8px",
    // Design pass, Step 5: a near-opaque (not fully solid) background plus
    // backdrop-filter blur — see Questions.md item 35 for why this specific
    // treatment was chosen over deeper per-site theme detection: it adapts
    // to whatever's visually behind the banner on ANY site automatically,
    // without this code ever reading that site's own DOM/theme, so it adds
    // no interception-side fragility at all.
    background: `${bg}E6`,
    backdropFilter: "blur(6px)",
    WebkitBackdropFilter: "blur(6px)",
    color: "#FFFFFF",
    padding: "10px 18px",
    borderRadius: "8px",
    fontSize: "13px",
    fontFamily: "system-ui, -apple-system, sans-serif",
    zIndex: 2147483647,
    boxShadow: "0 4px 18px rgba(0,0,0,0.28)",
    maxWidth: "520px",
    textAlign: "left",
    lineHeight: "1.4",
    animation: reduceMotion ? "none" : "lango-fade-in 180ms ease-out",
    transition: reduceMotion ? "none" : "opacity 150ms ease-in, transform 150ms ease-in",
    // Not pointer-events: none — a blocked/error banner should stay
    // readable and selectable, not fight with the page underneath it.
  });
  document.body.appendChild(el);

  const autoDismiss = opts.autoDismiss !== false;
  if (autoDismiss) {
    setTimeout(() => dismiss(el), opts.duration || 4000);
  }
  return el;
}

function dismiss(el) {
  if (!el || !el.parentNode) return;
  el.classList.add("lango-exiting");
  const remove = () => {
    if (el.parentNode) el.remove();
  };
  if (prefersReducedMotion()) {
    remove();
  } else {
    setTimeout(remove, 150);
  }
}

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

/**
 * Staged loading indicator for an in-flight scan — design pass, Step 5, per
 * the timing model in Questions.md item 35. Call `startScanIndicator(label)`
 * when a scan begins; call the returned handle's `.done(message, kind, opts)`
 * (show a terminal banner) or `.clear()` (dismiss silently — the clean-
 * response case, which stays deliberately silent) once it resolves.
 */
function startScanIndicator(label, phrases) {
  ensureStyles();
  let stage = "pending"; // pending -> indeterminate -> phrases
  let el = null;
  let phraseTimer = null;
  let phraseIndex = 0;
  let finished = false;

  function renderIndeterminate() {
    removeExisting();
    const reduceMotion = prefersReducedMotion();
    el = document.createElement("div");
    el.id = "lango-banner";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    el.innerHTML = reduceMotion
      ? `<span>${escapeHtml(label)}</span>`
      : `<span class="lango-spinner" aria-hidden="true"></span><span>${escapeHtml(label)}</span>`;
    Object.assign(el.style, {
      position: "fixed",
      bottom: "100px",
      left: "50%",
      transform: "translateX(-50%)",
      display: "flex",
      alignItems: "center",
      gap: "8px",
      background: `${LANGO_BANNER_COLORS.neutral}E6`,
      backdropFilter: "blur(6px)",
      WebkitBackdropFilter: "blur(6px)",
      color: "#FFFFFF",
      padding: "10px 18px",
      borderRadius: "8px",
      fontSize: "13px",
      fontFamily: "system-ui, -apple-system, sans-serif",
      zIndex: 2147483647,
      boxShadow: "0 4px 18px rgba(0,0,0,0.28)",
      maxWidth: "520px",
      lineHeight: "1.4",
      animation: reduceMotion ? "none" : "lango-fade-in 180ms ease-out",
    });
    document.body.appendChild(el);
  }

  function renderPhrase() {
    if (!el) return;
    const textEl = el.querySelector("span:last-child");
    if (textEl) textEl.textContent = phrases[phraseIndex % phrases.length];
  }

  // Stage 1 (0-1000ms): nothing. A sub-second wait showing an indicator
  // reads as noise, not information — this is the common case for prompt
  // scanning after the performance pass's Step 3 fixes (measured ~400-900ms
  // warm), so most real prompt scans now show NO loading indicator at all,
  // by design.
  const t1 = setTimeout(() => {
    if (finished) return;
    stage = "indeterminate";
    renderIndeterminate();
  }, 1000);

  // Stage 2 (past ~3000ms): rotating honest status phrases instead of one
  // static label — the case this task called out specifically (response
  // scanning, still ~8-9s even after Step 3's fix).
  const t2 = setTimeout(() => {
    if (finished) return;
    stage = "phrases";
    if (!el) renderIndeterminate();
    phraseIndex = 0;
    renderPhrase();
    phraseTimer = setInterval(() => {
      phraseIndex += 1;
      renderPhrase();
    }, 2500);
  }, 3000);

  function stopTimers() {
    finished = true;
    clearTimeout(t1);
    clearTimeout(t2);
    if (phraseTimer) clearInterval(phraseTimer);
  }

  return {
    done(message, kind, opts) {
      stopTimers();
      showBanner(message, kind, opts);
    },
    clear() {
      stopTimers();
      if (el) dismiss(el);
    },
  };
}

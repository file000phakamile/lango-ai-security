#!/usr/bin/env node
// Real automated test (chat feature, Phase 5): confirms the extension's
// content scripts genuinely, deliberately never activate on this app's own
// deployed web domain — the native chat feature (see /chat) is this
// product's own surface; the extension exists for every OTHER site.
//
// No test framework existed anywhere in this repo before this file (no
// jest/vitest/playwright config, no `test` script — checked directly, see
// Questions.md). Rather than either (a) skipping automated coverage
// entirely, or (b) pulling in a full test framework for one assertion, this
// is a small, dependency-free Node script using only the built-in `assert`
// module — implements Chrome's actual match-pattern semantics (scheme,
// host, path, `*` wildcards) closely enough to answer the real question
// this needs to answer, rather than a superficial string-contains check
// that could pass while the real matching behavior is still wrong.
//
// Run: node extension/test/manifest-domain-exclusion.test.js
"use strict";

const assert = require("assert");
const fs = require("fs");
const path = require("path");

const MANIFEST_PATH = path.join(__dirname, "..", "manifest.json");

// This app's own deployed frontend domain(s) — the page(s) a user actually
// browses to. NOT the backend API host (lango-backend-qwkx.onrender.com),
// which legitimately belongs in host_permissions (the extension's
// background script fetches it) but must never be a content_scripts
// target — an API host is not a page with a DOM to inject a content script
// into.
const OWN_APP_DOMAINS = ["lango-app-dusky.vercel.app"];

/**
 * A real (if simplified) implementation of Chrome's match pattern algorithm
 * for a subset relevant here: <scheme>://<host>/<path>, where host and path
 * may contain literal `*` wildcards. Good enough to answer "would this
 * pattern match a request to this exact origin" correctly for both this
 * repo's real patterns (which only ever use a trailing `/*` path wildcard)
 * and any hypothetical future one this test constructs.
 * See https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns
 */
function matchPatternToRegExp(pattern) {
  const m = pattern.match(/^(\*|https?|file|ftp):\/\/(\*|(?:\*\.)?[^/*]+|)(\/.*)$/);
  assert(m, `not a well-formed match pattern: ${pattern}`);
  const [, scheme, host, pathPart] = m;

  const schemeRe = scheme === "*" ? "https?" : scheme;
  let hostRe;
  if (host === "*") {
    hostRe = "[^/]+";
  } else if (host.startsWith("*.")) {
    hostRe = `(?:[^/]+\\.)?${escapeRegExp(host.slice(2))}`;
  } else {
    hostRe = escapeRegExp(host);
  }
  const pathRe = pathPart
    .split("*")
    .map(escapeRegExp)
    .join(".*");

  return new RegExp(`^${schemeRe}://${hostRe}${pathRe}$`);
}

function escapeRegExp(s) {
  return s.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
}

function patternMatchesUrl(pattern, url) {
  return matchPatternToRegExp(pattern).test(url);
}

function anyPatternMatches(patterns, url) {
  return (patterns || []).some((p) => patternMatchesUrl(p, url));
}

function run() {
  const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, "utf8"));
  let failures = 0;
  const check = (condition, message) => {
    try {
      assert(condition, message);
      console.log(`  ok - ${message}`);
    } catch (err) {
      failures++;
      console.error(`  FAIL - ${err.message}`);
    }
  };

  for (const domain of OWN_APP_DOMAINS) {
    const testUrl = `https://${domain}/`;
    console.log(`Checking exclusion for ${testUrl}`);

    // 1. host_permissions must never grant access to this app's own
    // frontend domain (it should have no reason to fetch itself, and
    // granting it would be a real, if quiet, mistake).
    check(
      !anyPatternMatches(manifest.host_permissions, testUrl),
      `host_permissions does not match ${testUrl}`,
    );

    // 2. For every content_scripts entry: if its own `matches` patterns
    // would activate on this domain, `exclude_matches` MUST explicitly
    // cancel that — this is the real, deliberate, load-bearing assertion,
    // not just "it happens not to match today".
    manifest.content_scripts.forEach((entry, i) => {
      const label = `content_scripts[${i}] (${entry.matches.join(", ")})`;
      const wouldMatch = anyPatternMatches(entry.matches, testUrl);
      const isExcluded = anyPatternMatches(entry.exclude_matches, testUrl);
      if (wouldMatch) {
        check(isExcluded, `${label}: matches ${testUrl} AND excludes it via exclude_matches`);
      } else {
        check(true, `${label}: does not match ${testUrl} at all`);
      }
      // Every entry must carry an explicit exclude_matches for this domain
      // regardless of whether its own site pattern happens to overlap
      // today — this is what makes the exclusion a deliberate, standing
      // guarantee rather than a coincidence of today's site list. If a 6th
      // content_scripts entry is ever added for a new AI chat site, this
      // assertion fails loudly unless that entry also excludes this domain.
      check(
        (entry.exclude_matches || []).some((p) => p.includes(domain)),
        `${label}: explicitly lists ${domain} in exclude_matches (deliberate, not coincidental)`,
      );
    });
  }

  // 3. Sanity check on the matcher itself: a hypothetical entry that DID
  // list this app's domain in `matches` must be caught by this test
  // (proves the assertions above aren't vacuously true because nothing
  // ever matches anything).
  const hypotheticalEntry = {
    matches: ["https://lango-app-dusky.vercel.app/*"],
    exclude_matches: [],
  };
  check(
    anyPatternMatches(hypotheticalEntry.matches, "https://lango-app-dusky.vercel.app/") &&
      !anyPatternMatches(hypotheticalEntry.exclude_matches, "https://lango-app-dusky.vercel.app/"),
    "self-check: the matcher correctly flags a hypothetical unexcluded entry as a real match (proves check #2 is not vacuous)",
  );

  console.log(failures === 0 ? "\nPASS" : `\nFAIL (${failures} assertion(s) failed)`);
  process.exit(failures === 0 ? 0 : 1);
}

run();

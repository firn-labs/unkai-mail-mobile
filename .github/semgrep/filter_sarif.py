#!/usr/bin/env python3
"""Drop reviewed findings from Semgrep's SARIF before it is uploaded.

    python3 filter_sarif.py <semgrep output> <filtered output>

Exits 1 when any finding is left after filtering, so the job stays a
gate: a new finding fails it, a reviewed one does not.

Two kinds of finding are dropped, and the log names every one of them:

1. **Findings carrying a `nosemgrep` marker.** Semgrep does not leave
   these out of its SARIF; it emits them with a `suppressions` entry
   (`[{"state": "accepted"}]`), and that entry lacks the `kind` field
   SARIF requires, so GitHub ignores it and opens an alert anyway.
   Without this step a justified `// nosemgrep: <rule>` in the source
   silenced nothing in the Security tab.

2. **The rule/path pairs in `REVIEWED` below** — a rule whose every
   hit in one place is expected by design, where a marker per hit
   would bury the code. Each entry says why it is safe to drop, and
   should name what keeps that true.
"""

import json
import sys

# (rule id, path prefix, why it is dropped there)
REVIEWED = [
    (
        "rust.lang.security.unsafe-usage.unsafe-usage",
        "crates/unkai-system/src/apple/",
        "the Objective-C bridge: objc2's bindings mark framework calls "
        "unsafe; the workspace `unsafe_code` lint (root Cargo.toml) "
        "keeps unsafe out of every other module, so this rule still "
        "reports any unsafe written anywhere else",
    ),
]


def location(result):
    """`path:line` of a SARIF result, for the log."""
    try:
        loc = result["locations"][0]["physicalLocation"]
        return f'{loc["artifactLocation"]["uri"]}:{loc["region"]["startLine"]}'
    except (KeyError, IndexError):
        return "<no location>"


def is_suppressed_in_source(result):
    """True for a finding Semgrep matched but a `nosemgrep` silenced.

    A suppression still under review or rejected does not count.
    Semgrep writes the status as `state`; SARIF names it `status`.
    """
    for suppression in result.get("suppressions") or []:
        status = suppression.get("status", suppression.get("state", "accepted"))
        if status == "accepted":
            return True
    return False


def reviewed_reason(result):
    """The `REVIEWED` reason covering this finding, or None."""
    rule = result.get("ruleId", "")
    path = location(result)
    for rule_id, prefix, why in REVIEWED:
        if rule == rule_id and path.startswith(prefix):
            return why
    return None


def main(src, dst):
    with open(src, encoding="utf-8") as f:
        sarif = json.load(f)

    kept_total = 0
    for run in sarif.get("runs", []):
        kept = []
        marked = []
        by_reason = {}
        for result in run.get("results", []):
            if is_suppressed_in_source(result):
                marked.append(result)
                continue
            why = reviewed_reason(result)
            if why is not None:
                by_reason.setdefault((result.get("ruleId"), why), []).append(result)
                continue
            kept.append(result)
        run["results"] = kept
        kept_total += len(kept)

        for result in marked:
            print(f"dropped (nosemgrep in source): {result.get('ruleId')} at {location(result)}")
        for (rule, why), results in by_reason.items():
            print(f"dropped {len(results)} x {rule} — {why}")
        for result in kept:
            print(f"FINDING: {result.get('ruleId')} at {location(result)}")

    with open(dst, "w", encoding="utf-8") as f:
        json.dump(sarif, f)

    print(f"{kept_total} finding(s) left after filtering")
    return 1 if kept_total else 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1], sys.argv[2]))

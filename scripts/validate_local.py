#!/usr/bin/env python3
"""Independent real JSONL versus SQLite validation; Python is only a dev tool.

Reads original logs without modifying them. Optional --output contains private
metadata and must remain outside the repository. Stdout only reports status.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import random
import sqlite3
import time
from datetime import datetime

FIELDS = ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens")


def read_counts(path):
    session = None
    identities = set()
    totals = collections.defaultdict(lambda: [0, 0, 0, 0])
    seen = set()
    previous = None
    epoch = 0
    modern = False
    fork_boundary = None
    with path.open("rb") as stream:
        for line in stream:
            if len(line) > 2 * 1024 * 1024:
                continue
            try:
                event = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            kind = event.get("type")
            p = event.get("payload") or {}
            if kind == "session_meta":
                if session is None:
                    session = p.get("id") or p.get("session_id")
                    identities.add(session)
                    if p.get("forked_from_id"):
                        fork_boundary = datetime.fromisoformat(event["timestamp"].replace("Z", "+00:00"))
            if kind == "token_usage_record":
                modern = True
                sid = p.get("thread_id") or p.get("session_id") or session
                usage, cumulative = p.get("usage"), p.get("thread_token_usage")
            elif kind == "event_msg" and p.get("type") == "token_count" and p.get("info"):
                sid = session
                usage = p["info"].get("last_token_usage")
                cumulative = p["info"].get("total_token_usage")
            else:
                continue
            if not usage:
                continue
            if fork_boundary and datetime.fromisoformat(event["timestamp"].replace("Z", "+00:00")) <= fork_boundary:
                # Inherited snapshot, not a new inference in the fork.
                continue
            if cumulative:
                cumulative_total = cumulative.get("input_tokens", 0) + cumulative.get("output_tokens", 0)
                if sid == session:
                    if previous is not None and cumulative_total < previous:
                        epoch += 1
                    previous = cumulative_total
                key = (sid, epoch, *(cumulative.get(k, 0) for k in FIELDS))
            else:
                key = (sid, event.get("timestamp"), *(usage.get(k, 0) for k in FIELDS))
            if key in seen:
                continue
            seen.add(key)
            values = [usage.get(k, 0) for k in FIELDS]
            assert 0 <= values[1] <= values[0]
            assert 0 <= values[3] <= values[2]
            totals[sid] = [a + b for a, b in zip(totals[sid], values)]
    return identities, totals, modern


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--codex-home", type=Path, default=Path(os.environ.get("CODEX_HOME", Path.home()/".codex")))
    ap.add_argument("--database", type=Path, default=Path.home()/"Library/Application Support/Codex Unified Monitor/monitor.sqlite3")
    ap.add_argument("--output", type=Path)
    ap.add_argument("--samples", type=int, default=5)
    ap.add_argument("--modern", action="store_true", help="Require response-level records")
    args = ap.parse_args()
    files = sorted((args.codex_home/"sessions").rglob("*.jsonl")) + sorted((args.codex_home/"archived_sessions").glob("*.jsonl"))
    files = [f for f in files if time.time()-f.stat().st_mtime > 180]
    random.Random(20260910).shuffle(files)
    db = sqlite3.connect(f"file:{args.database}?mode=ro", uri=True)
    results = []
    used = set()
    for path in files:
        identities, totals, modern = read_counts(path)
        if args.modern and not modern:
            continue
        if len(identities) != 1 or len(totals) != 1:
            continue
        sid, expected = next(iter(totals.items()))
        if sid in used or not sum(expected):
            continue
        used.add(sid)
        actual = list(db.execute("SELECT COALESCE(SUM(raw_input),0),COALESCE(SUM(cached_input),0),COALESCE(SUM(output),0),COALESCE(SUM(reasoning),0) FROM events WHERE session_id=?", (sid,)).fetchone())
        results.append({"sample":len(results)+1, "session_hash":hashlib.sha256(sid.encode()).hexdigest()[:12], "modern_records":modern, "expected":expected, "actual":actual, "uncached_input":expected[0]-expected[1], "total":expected[0]+expected[2], "pass":actual==expected})
        if len(results) == args.samples:
            break
    assert len(results) == args.samples, "Not enough stable real sessions to validate"
    ok = all(r["pass"] for r in results)
    report = {"status":"PASS" if ok else "FAIL", "samples":results, "generated_at":int(time.time())}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("w") as output:
            os.chmod(args.output, 0o600)
            json.dump(report, output, indent=2)
    print(json.dumps({"status":report["status"], "sessions":len(results), "matched":sum(r["pass"] for r in results), "modern_samples":sum(r["modern_records"] for r in results)}))
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()

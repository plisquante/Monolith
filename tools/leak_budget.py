#!/usr/bin/env python3
"""Leak budget: how fast the live managed heap grows after warm-up, from an allocation profile.

    MONO_ALLOC_PROFILE=prof.json <game run>          # writes prof.json (and prof.json.<n> snapshots)
    python3 tools/leak_budget.py prof.json [--warmup-s 60] [--budget-mb-per-min 1.0]

The profile records every collection (`gcs`: time, live bytes after it, pause). After the warm-up (chunks being loaded
is growth that ends) the live bytes are fitted with a least-squares line. A slope above the budget is a leak: the
program exits with status 1. The budget of the plan is < 1 MiB/min over a 60-minute soak (docs/GC_PLAN.md).

Fewer than three collections after the warm-up is not a verdict: the tool says so and exits with status 2.
"""
import argparse, json, sys


def fit(points):
    """Least-squares slope of y over x. points: [(x, y)]. Returns (slope, intercept)."""
    n = len(points)
    mx = sum(x for x, _ in points) / n
    my = sum(y for _, y in points) / n
    sxx = sum((x - mx) ** 2 for x, _ in points)
    if sxx == 0:
        return 0.0, my
    sxy = sum((x - mx) * (y - my) for x, y in points)
    slope = sxy / sxx
    return slope, my - slope * mx


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("profile")
    ap.add_argument("--warmup-s", type=float, default=60.0, help="seconds to ignore at the start (chunk loading)")
    ap.add_argument("--budget-mb-per-min", type=float, default=1.0)
    a = ap.parse_args()

    prof = json.load(open(a.profile, encoding="utf-8"))
    recs = prof.get("gcs") or []   # one record per collection: n, t_ms, live, heap, since, pause_us
    t = [r["t_ms"] for r in recs]
    live = [r["live"] for r in recs]
    pause = [r["pause_us"] for r in recs]
    if not t:
        print("leak budget: no collections in the profile (run longer, or lower MONO_ALLOC_DUMP_SECS)")
        return 2
    pts = [(ti / 60000.0, li / (1024 * 1024)) for ti, li in zip(t, live) if ti / 1000.0 >= a.warmup_s]
    print(f"collections: {len(t)}, after warm-up ({a.warmup_s:.0f} s): {len(pts)}")
    if pts:
        print(f"live after the first collection: {live[0] / 1048576:.1f} MiB, last: {live[-1] / 1048576:.1f} MiB")
        print(f"longest pause: {max(pause) / 1000:.1f} ms, mean: {sum(pause) / len(pause) / 1000:.1f} ms")
    if len(pts) < 3:
        print("not enough collections after the warm-up for a slope; run longer")
        return 2
    slope, _ = fit(pts)  # MiB per minute
    verdict = "WITHIN BUDGET" if slope <= a.budget_mb_per_min else "LEAK: above budget"
    print(f"live growth after warm-up: {slope:+.3f} MiB/min (budget {a.budget_mb_per_min:.2f}) -> {verdict}")
    return 0 if slope <= a.budget_mb_per_min else 1


if __name__ == "__main__":
    sys.exit(main())

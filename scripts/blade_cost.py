"""Rank blade skins by their GPU cost (Sol, 31/10/2026: find the top-tier blades that eat the
frame).

Runs `signature_gpu_cost_against_stock` (crates/sjk-viewer/src/blade_signatures_cost_test.rs)
once per scene (`game`: 16 players and one blade up close; `close`; `along`, the blade seen
down its length), every frame timed on the GPU with timestamp queries at 1920x1080, then
writes a ranked report: each blade's milliseconds per frame and its multiple of the stock
blade in every scene, worst first.

    python scripts/blade_cost.py                      # Legendary + Mythical
    python scripts/blade_cost.py --tiers all          # every blade
    python scripts/blade_cost.py --ids sun,void       # only those (saber_ left out)
    python scripts/blade_cost.py --scenes game,along

The five Legendary/Mythical blades with no signature (Void, Molten, Spectral, Clone Wars,
Mystery) are read from the hub's packs: --packs (default S:/Git/SJK-hub/assets/dist, or
SJK_TEST_PACKS). Opens no game window and needs no game data. Times are this machine's, with
whatever else uses the GPU: compare blades within one run, not runs across machines.
"""

import argparse
import csv
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCENES = ["game", "close", "along"]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--tiers", default="legendary,mythical")
    parser.add_argument("--ids", help="blade ids without saber_, comma-separated")
    parser.add_argument("--scenes", default=",".join(SCENES))
    parser.add_argument(
        "--packs",
        default=os.environ.get("SJK_TEST_PACKS", "S:/Git/SJK-hub/assets/dist"),
    )
    parser.add_argument("--out", default=str(ROOT / "target" / "blade-cost"))
    args = parser.parse_args()

    # absolute: the test runs from the crate's folder
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    table = out / "blade-cost.csv"
    table.unlink(missing_ok=True)
    env = dict(os.environ, SJK_COST_CSV=str(table))
    if args.ids:
        env["SJK_SIGNATURES"] = args.ids
    else:
        env["SJK_COST_TIERS"] = args.tiers
    if Path(args.packs).is_dir():
        env["SJK_TEST_PACKS"] = args.packs
    else:
        print(f"no packs at {args.packs}: blades without a signature are skipped")

    scenes = [scene.strip() for scene in args.scenes.split(",") if scene.strip()]
    for scene in scenes:
        print(f"== scene {scene}", flush=True)
        env["SJK_COST_SCENE"] = scene
        run = subprocess.run(
            [
                "cargo", "test", "--release", "-p", "sjk-viewer",
                "signature_gpu_cost_against_stock",
                "--", "--ignored", "--nocapture", "--test-threads=1",
            ],
            cwd=ROOT,
            env=env,
        )
        if run.returncode != 0:
            sys.exit(f"the {scene} scene failed")

    write_report(table, scenes, out / "blade-cost.md")


def write_report(table, scenes, report):
    blades = {}
    stock = {}
    with open(table, newline="", encoding="utf-8") as file:
        for row in csv.DictReader(file):
            blade = blades.setdefault(
                row["id"],
                {key: row[key] for key in ("number", "name", "tier", "passes")},
            )
            blade[row["scene"]] = (float(row["ms"]), float(row["x_stock"]))
            stock[row["scene"]] = float(row["stock_ms"])

    def worst(blade):
        return max(blade.get(scene, (0, 0))[1] for scene in scenes)

    ranked = sorted(blades.values(), key=worst, reverse=True)
    lines = [
        "# Blade GPU cost",
        "",
        "Milliseconds per frame (x the stock blade), 1920x1080, ranked by the worst scene.",
        "Stock blade: "
        + ", ".join(f"{scene} {stock[scene]:.4f} ms" for scene in scenes if scene in stock)
        + ".",
        "",
        "| # | No. | Blade | Tier | " + " | ".join(scenes) + " | Passes |",
        "|---|---|---|---|" + "---|" * len(scenes) + "---|",
    ]
    for rank, blade in enumerate(ranked, 1):
        cells = []
        for scene in scenes:
            if scene in blade:
                ms, times = blade[scene]
                cells.append(f"{ms:.4f} ({times:.1f}x)")
            else:
                cells.append("")
        lines.append(
            f"| {rank} | {blade['number']} | {blade['name']} | {blade['tier']} | "
            + " | ".join(cells)
            + f" | {blade['passes']} |"
        )
    report.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines[:8 + min(15, len(ranked))]))
    print(f"\nreport: {report}\ntable: {table}")


if __name__ == "__main__":
    main()

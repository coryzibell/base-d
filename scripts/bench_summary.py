#!/usr/bin/env python3
"""Summarize criterion benchmark results."""

import json
import sys
from pathlib import Path

CRITERION_DIR = Path("target/criterion")

def get_throughput(mean_ns: float, size_bytes: int) -> tuple[float, str]:
    """Calculate throughput from mean time. Returns (mb_per_sec, formatted_str)."""
    seconds = mean_ns / 1_000_000_000
    mb_per_sec = (size_bytes / seconds) / (1024 * 1024)
    if mb_per_sec >= 1000:
        return mb_per_sec, f"{mb_per_sec / 1024:.2f} GB/s"
    return mb_per_sec, f"{mb_per_sec:.1f} MB/s"

def parse_estimates(path: Path) -> dict | None:
    """Parse estimates.json file."""
    try:
        with open(path / "new" / "estimates.json") as f:
            data = json.load(f)
            return {
                "mean_ns": data["mean"]["point_estimate"],
                "std_dev_ns": data["std_dev"]["point_estimate"],
            }
    except (FileNotFoundError, json.JSONDecodeError, KeyError):
        return None

def summarize_dictionary(group_dir: Path) -> dict:
    """Summarize results for a dictionary/operation group."""
    results = {}
    paths = ["Scalar", "LUT", "Specialized"]
    sizes = [64, 256, 1024, 4096, 16384, 65536]

    for path in paths:
        path_dir = group_dir / path
        if not path_dir.exists():
            continue

        results[path] = {}
        for size in sizes:
            size_dir = path_dir / str(size)
            if size_dir.exists():
                estimates = parse_estimates(size_dir)
                if estimates:
                    mb_per_sec, formatted = get_throughput(estimates["mean_ns"], size)
                    results[path][size] = {
                        **estimates,
                        "throughput": formatted,
                        "throughput_mbps": mb_per_sec,
                    }

    return results

def print_full_table():
    """Print full benchmark comparison table."""
    if not CRITERION_DIR.exists():
        print("No benchmark results found. Run: cargo bench")
        sys.exit(1)

    groups = sorted(CRITERION_DIR.iterdir())

    # Collect all results
    all_results = {}
    for group_dir in groups:
        if not group_dir.is_dir() or group_dir.name == "report":
            continue
        all_results[group_dir.name] = summarize_dictionary(group_dir)

    # Print compact table
    print("\n" + "=" * 90)
    print("base-d Benchmark Summary (64KB input)")
    print("=" * 90)
    print(f"{'Operation/Dictionary':<25} {'Scalar':<12} {'LUT':<12} {'Specialized':<12} {'Best':<10}")
    print("-" * 90)

    for name, results in sorted(all_results.items()):
        if not results:
            continue

        row = f"{name:<25}"
        best_path = None
        best_throughput = 0

        for path in ["Scalar", "LUT", "Specialized"]:
            if path in results and 65536 in results[path]:
                tp = results[path][65536]["throughput"]
                tp_mbps = results[path][65536]["throughput_mbps"]
                row += f" {tp:<12}"
                if tp_mbps > best_throughput:
                    best_throughput = tp_mbps
                    best_path = path
            else:
                row += f" {'-':<12}"

        if best_path:
            row += f" {best_path}"
        print(row)

    print("-" * 90)

def print_detailed():
    """Print detailed per-dictionary summary."""
    if not CRITERION_DIR.exists():
        print("No benchmark results found. Run: cargo bench")
        sys.exit(1)

    groups = sorted(CRITERION_DIR.iterdir())

    print("\n" + "=" * 80)
    print("base-d Benchmark Results (Detailed)")
    print("=" * 80)

    for group_dir in groups:
        if not group_dir.is_dir() or group_dir.name == "report":
            continue

        print(f"\n{group_dir.name}")
        print("-" * 60)

        results = summarize_dictionary(group_dir)
        if not results:
            print("  No results yet")
            continue

        # Print table header
        print(f"  {'Path':<12} {'64B':<10} {'1KB':<10} {'64KB':<10}")
        print(f"  {'-'*12} {'-'*10} {'-'*10} {'-'*10}")

        for path in ["Scalar", "LUT", "Specialized"]:
            if path not in results:
                continue

            row = f"  {path:<12}"
            for size in [64, 1024, 65536]:
                if size in results[path]:
                    tp = results[path][size]["throughput"]
                    row += f" {tp:<10}"
                else:
                    row += f" {'-':<10}"
            print(row)

        # Calculate speedup
        if "Scalar" in results and 65536 in results["Scalar"]:
            scalar_ns = results["Scalar"][65536]["mean_ns"]
            print(f"\n  Speedup vs Scalar (64KB):")
            for path in ["LUT", "Specialized"]:
                if path in results and 65536 in results[path]:
                    speedup = scalar_ns / results[path][65536]["mean_ns"]
                    print(f"    {path}: {speedup:.1f}x")

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--detailed":
        print_detailed()
    else:
        print_full_table()

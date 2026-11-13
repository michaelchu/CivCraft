#!/usr/bin/env python3
"""Generate source-derived combat win-probability vectors.

Example:
  python3 tools/generate_freeciv_combat_oracles.py \
    --reference /Users/michaelchu/Documents/projects/CivCraft/freeciv

The generator extracts the pinned ``common/combat.c::win_chance`` function
verbatim, compiles it with a small controlled-input harness, and writes the
results to ``tests/fixtures/freeciv_combat_vectors.tsv``. It does not build or
run the Freeciv server.
"""

import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile


PINNED_REVISION = "9ff184b6eb128ca636c7ad6269e27aa959600006"
SOURCE_RELATIVE_PATH = Path("common/combat.c")
OUTPUT_DEFAULT = Path("tests/fixtures/freeciv_combat_vectors.tsv")

# Each tuple follows win_chance(as, ahp, afp, ds, dhp, dfp):
# attacker strength/HP/firepower, then defender strength/HP/firepower.
CASES = [
    ("symmetric_one_hp_per_round", 10, 10, 1, 10, 10, 1),
    ("attacker_strength_advantage", 20, 10, 1, 10, 10, 1),
    ("defender_strength_advantage", 10, 10, 1, 20, 10, 1),
    ("attacker_extra_hit_points", 10, 20, 1, 10, 10, 1),
    ("defender_extra_hit_points", 10, 10, 1, 10, 20, 1),
    ("attacker_higher_firepower", 10, 10, 2, 10, 10, 1),
    ("defender_higher_firepower", 10, 10, 1, 10, 10, 2),
    ("asymmetric_strength_hp_firepower", 12, 10, 3, 8, 7, 2),
    ("single_hit_attacker", 10, 1, 1, 10, 10, 1),
    ("zero_strength_uses_equal_round_odds", 0, 10, 1, 0, 10, 1),
    ("extreme_attacker_advantage", 1000, 1, 1, 1, 1, 1),
    ("extreme_defender_advantage", 1, 1, 1, 1000, 20, 3),
]


def extract_function(source: str) -> str:
    signature = "double win_chance(int as, int ahp, int afp, int ds, int dhp, int dfp)"
    start = source.find(signature)
    if start < 0:
        raise ValueError(f"could not find pinned function signature: {signature}")
    open_brace = source.find("{", start + len(signature))
    if open_brace < 0:
        raise ValueError("could not find win_chance function body")

    depth = 1
    position = open_brace + 1
    while depth:
        if position >= len(source):
            raise ValueError("unterminated win_chance function body")
        char = source[position]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        position += 1
    return source[start:position]


def compile_and_run(function: str, compiler: str) -> list[str]:
    harness = r"""
#include <math.h>
#include <stdio.h>

__FUNCTION__

int main(void)
{
  int as, ahp, afp, ds, dhp, dfp;
  while (scanf("%d %d %d %d %d %d", &as, &ahp, &afp, &ds, &dhp, &dfp) == 6) {
    printf("%.17g\n", win_chance(as, ahp, afp, ds, dhp, dfp));
  }
  return 0;
}
""".replace("__FUNCTION__", function)

    inputs = "\n".join(" ".join(map(str, case[1:])) for case in CASES) + "\n"
    with tempfile.TemporaryDirectory(prefix="freeciv-combat-oracle-") as temp_dir:
        temp = Path(temp_dir)
        c_file = temp / "combat_oracle.c"
        executable = temp / "combat_oracle"
        c_file.write_text(harness)
        subprocess.run(
            [compiler, "-std=c11", "-O2", str(c_file), "-lm", "-o", str(executable)],
            check=True,
        )
        result = subprocess.run(
            [str(executable)],
            input=inputs.encode(),
            check=True,
            capture_output=True,
        )
    values = result.stdout.decode().splitlines()
    if len(values) != len(CASES):
        raise RuntimeError(f"oracle returned {len(values)} values for {len(CASES)} cases")
    return values


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--reference",
        type=Path,
        required=True,
        help="pinned Freeciv source checkout at revision " + PINNED_REVISION,
    )
    parser.add_argument("--output", type=Path, default=OUTPUT_DEFAULT)
    parser.add_argument(
        "--compiler",
        default=shutil.which("clang") or "clang",
        help="C compiler used for the extracted function (default: clang)",
    )
    args = parser.parse_args()

    reference = args.reference.resolve()
    revision = subprocess.check_output(
        ["git", "-C", str(reference), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != PINNED_REVISION:
        raise SystemExit(
            f"reference checkout is {revision}, expected pinned revision {PINNED_REVISION}"
        )

    source_path = reference / SOURCE_RELATIVE_PATH
    source = source_path.read_text()
    function = extract_function(source)
    function_sha256 = hashlib.sha256(function.encode()).hexdigest()
    probabilities = compile_and_run(function, args.compiler)

    lines = [
        "# Freeciv source-derived combat attacker-win probabilities.",
        f"# Source revision: {revision}",
        "# Source function: common/combat.c::win_chance(int as, int ahp, int afp, int ds, int dhp, int dfp)",
        f"# Extracted function SHA-256: {function_sha256}",
        "# Inputs: as=attacker strength, ahp=attacker HP, afp=attacker firepower; ds/dhp/dfp are defender values.",
        "# Output: probability that the attacker wins, from the unmodified C function.",
        "# case\tas\tahp\tafp\tds\tdhp\tdfp\tattacker_win_probability",
    ]
    for case, probability in zip(CASES, probabilities, strict=True):
        name, *inputs = case
        lines.append("\t".join([name, *(str(value) for value in inputs), probability]))

    output = args.output
    if not output.is_absolute():
        output = Path.cwd() / output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n")
    print(f"Wrote {len(CASES)} source-derived vectors to {output}")
    print(f"Source revision: {revision}")
    print(f"Extracted function SHA-256: {function_sha256}")


if __name__ == "__main__":
    main()

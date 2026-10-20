#!/usr/bin/env python3
"""Creates the reference rendering of a PDF fixture with the host build.

The reference rendering is the baseline of the image based regression test in
`scripts/verify-android-arm64-pdf.sh`. It is produced by the *same* MuPDF
version, built for the host platform, so that the ARM64 Android rendering can
be compared pixel by pixel against it.

The script renders with the host `mutool` if it is available, and falls back to
the MuPDF based harness otherwise. The output is a directory of PPM files named
`page-<index>.ppm`, which is the format the comparison script reads.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tempfile


def render_with_mutool(mutool: pathlib.Path, fixture: pathlib.Path, output: pathlib.Path) -> bool:
    """Renders the fixture with `mutool draw` and converts the result to PNG.

    `mutool` numbers its output files starting at 1. The reference pages are
    renamed to a zero based index, so that they match the file names of the
    verification harness.
    """
    with tempfile.TemporaryDirectory() as temporary:
        temporary_path = pathlib.Path(temporary)
        result = subprocess.run(
            [
                str(mutool),
                "draw",
                "-r",
                "72",
                "-o",
                str(temporary_path / "page-%d.png"),
                str(fixture),
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"mutool failed: {result.stderr}", file=sys.stderr)
            return False
        produced = sorted(temporary_path.glob("page-*.png"))
        if not produced:
            return False
        output.mkdir(parents=True, exist_ok=True)
        for index, page in enumerate(produced):
            (output / f"page-{index}.png").write_bytes(page.read_bytes())
        return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, help="the PDF fixture to render")
    parser.add_argument("--output", required=True, help="directory for the reference pages")
    parser.add_argument(
        "--mutool",
        default="mutool",
        help="path to the host mutool binary (optional)",
    )
    arguments = parser.parse_args()

    fixture = pathlib.Path(arguments.fixture)
    output = pathlib.Path(arguments.output)
    if not fixture.is_file():
        print(f"ERROR: fixture not found: {fixture}", file=sys.stderr)
        return 1

    mutool = shutil_which(arguments.mutool)
    if mutool is None:
        print(
            "ERROR: no host 'mutool' found. Install MuPDF on the host to create the "
            "reference rendering.",
            file=sys.stderr,
        )
        return 1

    if not render_with_mutool(mutool, fixture, output):
        print("ERROR: the reference rendering could not be produced", file=sys.stderr)
        return 1

    pages = sorted(output.glob("page-*.png"))
    print(f"OK: {len(pages)} reference page(s) written to {output}")
    return 0


def shutil_which(command: str) -> pathlib.Path | None:
    """Resolves a command in PATH, mirroring `shutil.which` without the import."""
    import os
    import shutil

    found = shutil.which(command)
    return pathlib.Path(found) if found else None


if __name__ == "__main__":
    sys.exit(main())

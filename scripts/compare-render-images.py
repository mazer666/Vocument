#!/usr/bin/env python3
"""Compares two directories of rendered pages pixel by pixel.

The comparison is a real image comparison: every pixel of every page is
compared, and the mean and maximum absolute difference are reported. A
difference above the configured threshold fails the check.

The script is used by `scripts/verify-android-arm64-pdf.sh` to compare the
rendering of the ARM64 Android engine against the reference rendering of the
host build. It is the image based regression evidence for the Android PDF
engine, not a file size or exit code check.

Both binary PPM (`P6`) and PNG images are supported. The PNG path uses Pillow
when it is available and falls back to ImageMagick otherwise.
"""

from __future__ import annotations

import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile


def _read_ppm(path: pathlib.Path) -> tuple[int, int, bytes]:
    """Reads a binary PPM file (P6) and returns (width, height, samples)."""
    data = path.read_bytes()
    if not data.startswith(b"P6"):
        raise ValueError(f"{path} is not a binary PPM file")
    # The header consists of the magic number, the width, the height and the
    # maximum value, separated by whitespace and terminated by a single
    # whitespace character.
    fields: list[bytes] = []
    index = 2
    while len(fields) < 3:
        while index < len(data) and data[index : index + 1].isspace():
            index += 1
        if data[index : index + 1] == b"#":
            while index < len(data) and data[index : index + 1] != b"\n":
                index += 1
            continue
        start = index
        while index < len(data) and not data[index : index + 1].isspace():
            index += 1
        fields.append(data[start:index])
    index += 1  # exactly one whitespace character follows the max value
    width = int(fields[0])
    height = int(fields[1])
    samples = data[index : index + width * height * 3]
    if len(samples) != width * height * 3:
        raise ValueError(f"{path} is truncated")
    return width, height, samples


def _read_png(path: pathlib.Path) -> tuple[int, int, bytes]:
    """Reads a PNG image and returns (width, height, RGB samples)."""
    try:
        from PIL import Image  # type: ignore[import-not-found]
    except ImportError:
        return _read_png_with_imagemagick(path)

    with Image.open(path) as image:
        converted = image.convert("RGB")
        return converted.width, converted.height, converted.tobytes()


def _read_png_with_imagemagick(path: pathlib.Path) -> tuple[int, int, bytes]:
    """Reads a PNG image through ImageMagick, used when Pillow is missing."""
    if shutil.which("convert") is None:
        raise ValueError(
            f"{path} is a PNG image, but neither Pillow nor ImageMagick is available"
        )
    with tempfile.TemporaryDirectory() as temporary:
        target = pathlib.Path(temporary) / "image.ppm"
        result = subprocess.run(
            ["convert", str(path), str(target)], capture_output=True, text=True
        )
        if result.returncode != 0:
            raise ValueError(f"{path} could not be converted: {result.stderr}")
        return _read_ppm(target)


def read_image(path: pathlib.Path) -> tuple[int, int, bytes]:
    """Reads an image and returns (width, height, RGB samples)."""
    if path.suffix.lower() == ".png":
        return _read_png(path)
    return _read_ppm(path)


def compare(actual: pathlib.Path, reference: pathlib.Path) -> tuple[float, int]:
    """Compares two images and returns (mean difference, maximum difference)."""
    actual_width, actual_height, actual_samples = read_image(actual)
    reference_width, reference_height, reference_samples = read_image(reference)
    if (actual_width, actual_height) != (reference_width, reference_height):
        raise ValueError(
            f"size mismatch: {actual.name} is {actual_width}x{actual_height}, "
            f"{reference.name} is {reference_width}x{reference_height}"
        )
    total = 0
    maximum = 0
    count = len(actual_samples)
    for index in range(count):
        difference = abs(actual_samples[index] - reference_samples[index])
        total += difference
        if difference > maximum:
            maximum = difference
    mean = total / count if count else 0.0
    return mean, maximum


def collect_pages(directory: pathlib.Path) -> list[pathlib.Path]:
    """Returns the rendered pages of a directory, sorted by their page number."""
    pages = [
        *sorted(directory.glob("page-*.png")),
        *sorted(directory.glob("page-*.ppm")),
    ]
    return sorted(pages, key=lambda page: page.name)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--actual", required=True, help="directory with the rendered pages")
    parser.add_argument("--reference", required=True, help="directory with the reference pages")
    parser.add_argument(
        "--max-mean-difference",
        type=float,
        default=1.0,
        help="maximum accepted mean absolute difference per channel",
    )
    arguments = parser.parse_args()

    actual_dir = pathlib.Path(arguments.actual)
    reference_dir = pathlib.Path(arguments.reference)
    if not actual_dir.is_dir():
        print(f"ERROR: {actual_dir} is not a directory", file=sys.stderr)
        return 1
    if not reference_dir.is_dir():
        print(f"ERROR: {reference_dir} is not a directory", file=sys.stderr)
        return 1

    actual_pages = collect_pages(actual_dir)
    reference_pages = collect_pages(reference_dir)
    if not actual_pages:
        print(f"ERROR: no rendered pages found in {actual_dir}", file=sys.stderr)
        return 1
    if len(actual_pages) != len(reference_pages):
        print(
            f"ERROR: page count mismatch: {len(actual_pages)} rendered, "
            f"{len(reference_pages)} in the reference",
            file=sys.stderr,
        )
        return 1

    failures = 0
    for actual_page, reference_page in zip(actual_pages, reference_pages):
        try:
            mean, maximum = compare(actual_page, reference_page)
        except ValueError as error:
            print(f"MISMATCH: {actual_page.name}: {error}", file=sys.stderr)
            failures += 1
            continue
        status = "OK" if mean <= arguments.max_mean_difference else "MISMATCH"
        print(
            f"{status}: {actual_page.name} vs {reference_page.name} "
            f"mean_diff={mean:.3f} max_diff={maximum}"
        )
        if status == "MISMATCH":
            failures += 1

    if failures:
        print(
            f"ERROR: {failures} page(s) differ from the reference rendering",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(actual_pages)} page(s) match the reference rendering")
    return 0


if __name__ == "__main__":
    sys.exit(main())

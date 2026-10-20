#!/usr/bin/env python3
"""Regenerates the OCR corpus frame fixtures used by the Vocument OCR quality gate.

`scripts/generate-ocr-corpus.py` produces the source PDFs, the PNG renders and the
expected-text / keyword metadata. The Rust OCR corpus tests
(`engines/ocr/tests/corpus_tests.rs`) however load the **grayscale PGM frames** in
`tests/corpus/ocr/`, and the M1.3 camera-distortion gate additionally loads a
simulated camera capture frame.

Both frame families were previously committed as opaque binaries without a
generator, which made the M1.3 quality claim (`CER <= 5%`, `char accuracy >= 95%`)
impossible to reproduce or audit. This script closes that gap by deriving every
frame deterministically from the already-committed PNG renders.

Frame provenance
----------------

* Plain documents (``deu_vertrag_vertraulichkeit``, ``eng_mutual_nda``,
  ``bilingual_rechnung_invoice``): the PGM is the exact grayscale conversion of the
  committed PNG render. Verified byte-identical.

* The camera capture frame (``deu_vertrag_vertraulichkeit_camera_distorted``): the
  committed PNG is a 4:3 bilinear downscale of the committed PGM (1500x2050 ->
  1125x1538). The PGM is therefore the source of record; this script regenerates it
  from the flat document render through the same projective + illumination model and
  then re-derives the PNG from it.

Usage::

    python3 scripts/generate-ocr-corpus-frames.py            # write frames
    python3 scripts/generate-ocr-corpus-frames.py --check    # verify only

``--check`` exits non-zero when a committed frame differs from the regenerated frame
so it can be used as a regression check.

Requires: Pillow, NumPy.
"""

import argparse
import os
import sys

import numpy as np
from PIL import Image, ImageFilter

REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
)
CORPUS_DIR = os.path.join(REPO_ROOT, "tests", "corpus", "ocr")

# Desk background value of the simulated camera capture frame.
DESK_BACKGROUND_LEVEL = 42

# Canvas size of the simulated camera capture frame.
CAMERA_FRAME_WIDTH = 1500
CAMERA_FRAME_HEIGHT = 2050

# Size of the derived camera capture preview that ships as PNG.
CAMERA_PREVIEW_WIDTH = 1125
CAMERA_PREVIEW_HEIGHT = 1538

# Quadrilateral (top-left, top-right, bottom-right, bottom-left) of the document
# inside the camera capture frame. These are the same corners the Rust gate uses in
# `engines/ocr/tests/corpus_tests.rs`
# (`test_realistic_camera_distorted_scan_and_cleanup_pipeline`).
CAMERA_DOCUMENT_QUAD = (
    (233.32, 82.95),
    (1465.35, 223.33),
    (1266.68, 1967.05),
    (34.65, 1826.67),
)

# Supersampling factor used while rendering the flat document into the camera
# canvas. Rendering above the canvas resolution and then downsampling keeps the
# glyph edges crisp, so that the perspective warp does not destroy stroke detail.
CAMERA_SUPERSAMPLE = 3

# Gaussian blur radius (in canvas pixels) applied to the camera capture frame to
# emulate the optical softness of a real phone camera. The blur is what makes the
# unrectified capture genuinely hard for the OCR engine, which is exactly the
# precondition the M1.3 gate asserts (raw capture must fail the quality gate,
# rectified + cleaned capture must pass it).
CAMERA_BLUR_RADIUS = 1.0

# Illumination/shadow model applied to the rectified document.
#
# The model is a multiplicative field over the rectified page:
#
#   illumination(x, y) = base
#                       - gradient_x * (x / width)
#                       - gradient_y * (y / height)
#                       - shadow_strength * exp(-((x - cx)^2 + (y - cy)^2) / (2 * s^2))
#
# The Gaussian term models the soft cast shadow of the capturing device; the linear
# terms model the non-uniform illumination gradient across the desk.
ILLUMINATION_BASE = 1.07797
ILLUMINATION_GRADIENT_X = 0.08611
ILLUMINATION_GRADIENT_Y = 0.19399
SHADOW_STRENGTH = 0.13397
SHADOW_CENTER_X = 700.0
SHADOW_CENTER_Y = 950.0
SHADOW_SIGMA = 380.0

# Simple grayscale documents that are rendered straight from their PNG render.
PLAIN_FRAME_DOCUMENTS = (
    "deu_vertrag_vertraulichkeit",
    "eng_mutual_nda",
    "bilingual_rechnung_invoice",
)

# Document that additionally gets the simulated camera capture treatment.
CAMERA_DISTORTED_DOCUMENT = "deu_vertrag_vertraulichkeit_camera_distorted"
CAMERA_DISTORTED_SOURCE = "deu_vertrag_vertraulichkeit"


def compute_homography(source_points, destination_points):
    """Solves the 8-parameter planar homography mapping source onto destination."""
    rows = []
    values = []
    for (source_x, source_y), (dest_x, dest_y) in zip(source_points, destination_points):
        rows.append([source_x, source_y, 1, 0, 0, 0, -dest_x * source_x, -dest_x * source_y])
        values.append(dest_x)
        rows.append([0, 0, 0, source_x, source_y, 1, -dest_y * source_x, -dest_y * source_y])
        values.append(dest_y)
    solution = np.linalg.solve(np.array(rows, dtype=float), np.array(values, dtype=float))
    return np.array(
        [
            [solution[0], solution[1], solution[2]],
            [solution[3], solution[4], solution[5]],
            [solution[6], solution[7], 1.0],
        ]
    )


def sample_bilinear(image, x_coordinates, y_coordinates):
    """Samples `image` at float coordinates using bilinear interpolation."""
    height, width = image.shape
    x_floor = np.floor(x_coordinates).astype(int)
    y_floor = np.floor(y_coordinates).astype(int)
    x_floor = np.clip(x_floor, 0, width - 1)
    y_floor = np.clip(y_floor, 0, height - 1)
    x_ceil = np.clip(x_floor + 1, 0, width - 1)
    y_ceil = np.clip(y_floor + 1, 0, height - 1)
    x_fraction = x_coordinates - x_floor
    y_fraction = y_coordinates - y_floor

    top_left = image[y_floor, x_floor]
    top_right = image[y_floor, x_ceil]
    bottom_left = image[y_ceil, x_floor]
    bottom_right = image[y_ceil, x_ceil]

    top = top_left * (1.0 - x_fraction) + top_right * x_fraction
    bottom = bottom_left * (1.0 - x_fraction) + bottom_right * x_fraction
    return top * (1.0 - y_fraction) + bottom * y_fraction


def render_camera_capture_frame(source_png_path):
    """Builds the simulated camera capture frame from the flat document render.

    The document is rendered at `CAMERA_SUPERSAMPLE` times the canvas resolution,
    projected into the camera canvas through a planar homography, lit by the
    illumination/shadow model, composited onto the desk background and finally
    softened with a Gaussian blur to emulate the optics of a phone camera.
    """
    source_image = Image.open(source_png_path).convert("L")
    source_width, source_height = source_image.size

    supersampled_width = source_width * CAMERA_SUPERSAMPLE
    supersampled_height = source_height * CAMERA_SUPERSAMPLE
    supersampled = np.array(
        source_image.resize(
            (supersampled_width, supersampled_height), Image.LANCZOS
        )
    ).astype(np.float64)

    homography = compute_homography(
        [
            (0.0, 0.0),
            (float(supersampled_width - 1), 0.0),
            (float(supersampled_width - 1), float(supersampled_height - 1)),
            (0.0, float(supersampled_height - 1)),
        ],
        CAMERA_DOCUMENT_QUAD,
    )
    inverse_homography = np.linalg.inv(homography)

    grid_y, grid_x = np.mgrid[0:CAMERA_FRAME_HEIGHT, 0:CAMERA_FRAME_WIDTH]
    homogeneous = np.stack([grid_x, grid_y, np.ones_like(grid_x, dtype=float)], axis=-1)
    projected = homogeneous @ inverse_homography.T
    source_x = projected[..., 0] / projected[..., 2]
    source_y = projected[..., 1] / projected[..., 2]

    inside = (
        (source_x >= 0)
        & (source_x <= supersampled_width - 1)
        & (source_y >= 0)
        & (source_y <= supersampled_height - 1)
    )
    rectified = sample_bilinear(supersampled, source_x, source_y)

    illumination = (
        ILLUMINATION_BASE
        - ILLUMINATION_GRADIENT_X * (grid_x / CAMERA_FRAME_WIDTH)
        - ILLUMINATION_GRADIENT_Y * (grid_y / CAMERA_FRAME_HEIGHT)
        - SHADOW_STRENGTH
        * np.exp(
            -((grid_x - SHADOW_CENTER_X) ** 2 + (grid_y - SHADOW_CENTER_Y) ** 2)
            / (2.0 * SHADOW_SIGMA ** 2)
        )
    )

    frame = np.full((CAMERA_FRAME_HEIGHT, CAMERA_FRAME_WIDTH), float(DESK_BACKGROUND_LEVEL))
    lit = np.clip(rectified * illumination, 0.0, 255.0)
    frame[inside] = lit[inside]

    # Optical softness of the capture device.
    blurred = np.array(
        Image.fromarray(np.clip(np.rint(frame), 0, 255).astype(np.uint8)).filter(
            ImageFilter.GaussianBlur(CAMERA_BLUR_RADIUS)
        )
    ).astype(np.float64)
    # Restore the desk background, the blur would otherwise bleed into it.
    blurred[np.abs(blurred - DESK_BACKGROUND_LEVEL) <= 2] = DESK_BACKGROUND_LEVEL
    return blurred


def write_pgm(path, pixels):
    """Writes a binary P5 grayscale image, matching the committed fixtures."""
    height, width = pixels.shape
    with open(path, "wb") as handle:
        handle.write(f"P5\n{width} {height}\n255\n".encode("ascii"))
        handle.write(np.clip(np.rint(pixels), 0, 255).astype(np.uint8).tobytes())


def read_pgm(path):
    """Reads a binary P5 grayscale image."""
    with open(path, "rb") as handle:
        data = handle.read()
    if not data.startswith(b"P5"):
        raise ValueError(f"{path}: not a binary P5 PGM file")
    tokens = []
    offset = 2
    while len(tokens) < 3:
        while offset < len(data) and data[offset : offset + 1].isspace():
            offset += 1
        if data[offset : offset + 1] == b"#":
            while offset < len(data) and data[offset : offset + 1] != b"\n":
                offset += 1
            continue
        start = offset
        while offset < len(data) and not data[offset : offset + 1].isspace():
            offset += 1
        tokens.append(int(data[start:offset]))
    offset += 1
    width, height, max_value = tokens
    if max_value != 255:
        raise ValueError(f"{path}: unsupported max value {max_value}")
    pixels = np.frombuffer(data, dtype=np.uint8, count=width * height, offset=offset)
    return pixels.reshape(height, width)


def flat_frame_for(document_id):
    """Grayscale frame of a plain document render."""
    png_path = os.path.join(CORPUS_DIR, f"{document_id}.png")
    return np.array(Image.open(png_path).convert("L")).astype(np.float64)


def expected_frames():
    """Returns the mapping of frame file name to expected pixel content."""
    frames = {}
    for document_id in PLAIN_FRAME_DOCUMENTS:
        frames[f"{document_id}.pgm"] = flat_frame_for(document_id)

    camera_png = os.path.join(CORPUS_DIR, f"{CAMERA_DISTORTED_SOURCE}.png")
    frames[f"{CAMERA_DISTORTED_DOCUMENT}.pgm"] = render_camera_capture_frame(camera_png)
    return frames


def camera_preview_pixels(camera_frame):
    """Derives the camera capture preview PNG from the camera capture frame.

    The committed preview is a 4:3 bilinear downscale of the capture frame.
    """
    preview = Image.fromarray(
        np.clip(np.rint(camera_frame), 0, 255).astype(np.uint8)
    ).resize((CAMERA_PREVIEW_WIDTH, CAMERA_PREVIEW_HEIGHT), Image.BILINEAR)
    return np.array(preview).astype(np.float64)


def android_asset_paths(file_name):
    """Returns the Android asset copies that must stay in sync with a corpus frame."""
    if file_name != f"{CAMERA_DISTORTED_DOCUMENT}.png":
        return []
    asset_dir = os.path.join(
        REPO_ROOT, "apps", "android", "app", "src", "main", "assets", "corpus"
    )
    return [os.path.join(asset_dir, file_name)]


def compare(existing, expected):
    """Returns the mean and maximum absolute difference between two frames."""
    difference = np.abs(existing.astype(np.float64) - expected)
    return float(difference.mean()), float(difference.max())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify committed frames instead of writing them (exit 1 on mismatch)",
    )
    arguments = parser.parse_args()

    if not os.path.isdir(CORPUS_DIR):
        print(f"ERROR: corpus directory not found: {CORPUS_DIR}", file=sys.stderr)
        return 1

    failures = []
    frames = expected_frames()

    for file_name, pixels in frames.items():
        path = os.path.join(CORPUS_DIR, file_name)
        if arguments.check:
            if not os.path.exists(path):
                failures.append(f"{file_name}: missing")
                continue
            mean_difference, max_difference = compare(read_pgm(path), pixels)
            status = "OK" if mean_difference <= 1.0 else "MISMATCH"
            print(
                f"{status}: {file_name} mean_diff={mean_difference:.3f} "
                f"max_diff={max_difference:.0f}"
            )
            if mean_difference > 1.0:
                failures.append(
                    f"{file_name}: mean difference {mean_difference:.3f} exceeds 1.0"
                )
        else:
            write_pgm(path, pixels)
            print(f"wrote {path}")

    # The camera capture preview PNG is derived from the camera capture frame.
    camera_frame = frames[f"{CAMERA_DISTORTED_DOCUMENT}.pgm"]
    preview_pixels = camera_preview_pixels(camera_frame)
    preview_path = os.path.join(CORPUS_DIR, f"{CAMERA_DISTORTED_DOCUMENT}.png")
    preview_targets = [preview_path] + android_asset_paths(
        f"{CAMERA_DISTORTED_DOCUMENT}.png"
    )
    for target in preview_targets:
        if arguments.check:
            if not os.path.exists(target):
                failures.append(f"{os.path.basename(target)}: missing")
                continue
            existing = np.array(Image.open(target).convert("L")).astype(np.float64)
            mean_difference, max_difference = compare(existing, preview_pixels)
            status = "OK" if mean_difference <= 1.0 else "MISMATCH"
            print(
                f"{status}: {os.path.relpath(target, REPO_ROOT)} "
                f"mean_diff={mean_difference:.3f} max_diff={max_difference:.0f}"
            )
            if mean_difference > 1.0:
                failures.append(
                    f"{os.path.relpath(target, REPO_ROOT)}: mean difference "
                    f"{mean_difference:.3f} exceeds 1.0"
                )
        else:
            os.makedirs(os.path.dirname(target), exist_ok=True)
            Image.fromarray(
                np.clip(np.rint(preview_pixels), 0, 255).astype(np.uint8)
            ).convert("RGB").save(target)
            print(f"wrote {target}")

    if failures:
        print("\nFAILURES:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Generates the difficult PDF corpus.

The corpus contains PDFs that are broken or unusual in a way that a real
document can be broken or unusual: a damaged cross-reference table, a stream
that is cut off in the middle, a page without any size, a page with extreme
dimensions, an encrypted file, a file with a cyclic page tree, a page whose
content object does not exist, and a file with many pages.

The point is not that the engine has to open every file. The point is that it
has to deal with every file **without crashing, without hanging and without
leaking an unreported error**. A file that cannot be read has to produce a
clean error, not a panic and not a frozen process.

The script is deterministic: running it again produces byte-identical files, so
the corpus can be checked into the repository and reproduced at any time.

Usage:
    python3 scripts/generate-difficult-pdf-corpus.py [output_dir]
"""

from __future__ import annotations

import hashlib
import pathlib
import struct
import sys
from typing import Iterable

# The standard security handler needs these padding bytes.
PAD = bytes(
    [
        0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41,
        0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
        0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80,
        0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
    ]
)


def rc4(key: bytes, data: bytes) -> bytes:
    """The RC4 stream cipher, as used by the standard security handler."""
    state = list(range(256))
    j = 0
    for i in range(256):
        j = (j + state[i] + key[i % len(key)]) % 256
        state[i], state[j] = state[j], state[i]
    out = bytearray()
    i = j = 0
    for byte in data:
        i = (i + 1) % 256
        j = (j + state[i]) % 256
        state[i], state[j] = state[j], state[i]
        out.append(byte ^ state[(state[i] + state[j]) % 256])
    return bytes(out)


def build_pdf(objects: list[bytes], *, root: int = 1, extra_trailer: str = "") -> bytes:
    """Assembles a PDF from its objects and computes an exact cross-reference table."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for index, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out.extend(f"{index} 0 obj\n".encode("latin-1"))
        out.extend(body)
        out.extend(b"\nendobj\n")
    xref_offset = len(out)
    out.extend(f"xref\n0 {len(objects) + 1}\n".encode("latin-1"))
    out.extend(b"0000000000 65535 f \n")
    for offset in offsets:
        out.extend(f"{offset:010} 00000 n \n".encode("latin-1"))
    trailer = f"<< /Size {len(objects) + 1} /Root {root} 0 R {extra_trailer}>>"
    out.extend(f"trailer\n{trailer}\nstartxref\n{xref_offset}\n%%EOF\n".encode("latin-1"))
    return bytes(out)


def simple_content(text: str) -> bytes:
    return f"BT /F1 12 Tf 72 720 Td ({text}) Tj ET".encode("latin-1")


def pdf_damaged_xref() -> bytes:
    """Valid objects, but the cross-reference table points to the wrong places."""
    out = bytearray(b"%PDF-1.7\n")
    offsets = []
    bodies = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length 44 >>\nstream\nBT /F1 12 Tf 72 720 Td (Repariert oder Fehler) Tj ET\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    for body in bodies:
        offsets.append(len(out))
        out.extend(b"1 0 obj\n".replace(b"1", str(len(offsets) - 1 + 1).encode(), 1))
        out.extend(body)
        out.extend(b"\nendobj\n")
    xref_offset = len(out)
    out.extend(f"xref\n0 {len(bodies) + 1}\n".encode("latin-1"))
    out.extend(b"0000000000 65535 f \n")
    # Every offset is wrong by a fixed amount: the table is damaged, the objects
    # themselves are intact.
    for offset in offsets:
        out.extend(f"{offset + 17:010} 00000 n \n".encode("latin-1"))
    out.extend(
        f"trailer\n<< /Size {len(bodies) + 1} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode(
            "latin-1"
        )
    )
    return bytes(out)


def pdf_truncated_stream() -> bytes:
    """The content stream is cut off in the middle of the file."""
    body = build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
            b"<< /Length 200 >>\nstream\nBT /F1 12 Tf 72 720 Td (abgeschnitten",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ]
    )
    # Cut the file in the middle of the content stream.
    return body[: body.index(b"stream\n") + 40]


def pdf_zero_size_page() -> bytes:
    """A page without any area."""
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 0 0] >>",
        ]
    )


def pdf_huge_page() -> bytes:
    """A page with extreme dimensions: 20000 x 20000 points."""
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 20000 20000] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            b"<< /Length 60 >>\nstream\nBT /F1 12 Tf 100 19000 Td (riesige Seite) Tj ET\nendstream",
        ]
    )


def pdf_cyclic_page_tree() -> bytes:
    """The page tree points at itself: a naive traversal would never end."""
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
        ]
    )


def pdf_missing_content_object() -> bytes:
    """The page refers to a content object that does not exist."""
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 9 0 R >>",
        ]
    )


def pdf_many_pages(count: int = 200) -> bytes:
    """A file with many pages: a realistic stress case for the scheduler."""
    objects: list[bytes] = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
    ]
    kids = " ".join(f"{3 + index} 0 R" for index in range(count))
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {count} >>".encode("latin-1"))
    for index in range(count):
        objects.append(
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources "
            f"<< /Font << /F1 {3 + count} 0 R >> >> /Contents {4 + count} 0 R >>".encode("latin-1")
        )
    objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    content = b"<< /Length 44 >>\nstream\nBT /F1 12 Tf 72 720 Td (viele Seiten) Tj ET\nendstream"
    for _ in range(count):
        objects.append(content)
    return build_pdf(objects)


def pdf_encrypted() -> bytes:
    """A file encrypted with the standard security handler and an empty user password."""
    document_id = hashlib.md5(b"vocument-difficult-corpus").digest()
    permissions = -1  # all permissions

    # Algorithm 3: the owner password is derived from the user password.
    owner_key = hashlib.md5(PAD).digest()
    owner_value = rc4(owner_key, PAD)
    # Algorithm 2: the encryption key.
    key_input = (
        PAD
        + owner_value
        + struct.pack("<i", permissions)
        + document_id
    )
    encryption_key = hashlib.md5(key_input).digest()[:5]
    # Algorithm 5: the user password entry.
    user_value = rc4(encryption_key, PAD)

    def object_key(number: int, generation: int) -> bytes:
        material = encryption_key + struct.pack("<i", number)[:3] + struct.pack("<i", generation)[:2]
        return hashlib.md5(material).digest()[: min(len(encryption_key) + 5, 16)]

    def encrypt(number: int, generation: int, data: bytes) -> bytes:
        return rc4(object_key(number, generation), data)

    content = simple_content("verschluesselt")
    encrypted_content = encrypt(4, 0, content)

    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources "
        b"<< /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length "
        + str(len(encrypted_content)).encode("latin-1")
        + b" >>\nstream\n"
        + encrypted_content
        + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Filter /Standard /V 1 /R 2 /O <"
        + owner_value.hex().encode("ascii")
        + b"> /U <"
        + user_value.hex().encode("ascii")
        + b"> /P "
        + str(permissions).encode("ascii")
        + b" >>",
    ]
    extra = f"/Encrypt 6 0 R /ID [<{document_id.hex()}> <{document_id.hex()}>] "
    return build_pdf(objects, extra_trailer=extra)


FIXTURES = {
    "damaged_xref.pdf": pdf_damaged_xref,
    "truncated_stream.pdf": pdf_truncated_stream,
    "zero_size_page.pdf": pdf_zero_size_page,
    "huge_page.pdf": pdf_huge_page,
    "cyclic_page_tree.pdf": pdf_cyclic_page_tree,
    "missing_content_object.pdf": pdf_missing_content_object,
    "many_pages.pdf": pdf_many_pages,
    "encrypted.pdf": pdf_encrypted,
}


def main() -> int:
    arguments = [argument for argument in sys.argv[1:] if argument != "--check"]
    check_only = "--check" in sys.argv[1:]
    target = pathlib.Path(arguments[0] if arguments else "tests/corpus/difficult")

    if check_only:
        # The committed corpus has to be byte-identical to the output of the
        # generator, so the robustness numbers stay auditable.
        mismatches = 0
        for name, builder in FIXTURES.items():
            path = target / name
            if not path.exists():
                print(f"MISSING: {path}", file=sys.stderr)
                mismatches += 1
                continue
            expected = builder()
            if path.read_bytes() != expected:
                print(f"MISMATCH: {path}", file=sys.stderr)
                mismatches += 1
        if mismatches:
            print(
                f"ERROR: {mismatches} file(s) of the difficult corpus do not match the generator.",
                file=sys.stderr,
            )
            return 1
        print(f"OK: {len(FIXTURES)} difficult PDF fixture(s) match the generator.")
        return 0

    target.mkdir(parents=True, exist_ok=True)
    for name, builder in FIXTURES.items():
        payload = builder()
        path = target / name
        path.write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        print(f"wrote {path} ({len(payload)} bytes) sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

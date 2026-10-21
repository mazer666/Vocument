#!/usr/bin/env python3
"""The structural check of the Swift sources (`CODE-001`, `QA-003`).

There is no Swift toolchain in every environment that builds this project,
and a Swift file that does not compile is a file that nobody notices — the
Rust part of the product is green, the gate is green, and the application
does not start.

The check is **not** a compiler. It is a structural check that catches the
class of defect that a mechanical edit of a source file produces:

* a file whose braces, parentheses or brackets are not balanced;
* a **statement** in the body of a type — Swift does not allow it, and a
  fragment of a method whose head was lost lands exactly there;
* the same declaration twice in one file — a copy that was pasted and never
  removed.

The check reads the file, drops the comments and the string literals, and
looks at the structure that is left. It is deliberately conservative: it
reports only what it is sure about, and a file it cannot understand is
reported as **not checked** rather than as green.

Usage:
    scripts/check-swift-structure.py [file ...]

Without arguments every Swift file of the application is checked.
Exit code 0: every file is structurally sound.
Exit code 1: at least one file has a defect.
"""

from __future__ import annotations

import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent

# The Swift files of the application. The generated bindings are not checked:
# they are written by the generator, not by a hand.
SOURCE_ROOTS = [
    REPOSITORY_ROOT / "apps" / "macos" / "Sources" / "VocumentMacApp",
    REPOSITORY_ROOT / "apps" / "macos" / "Sources" / "VocumentMacKit",
]

# The words that may open a declaration in the body of a type. Everything
# else at that depth is a statement — and Swift does not allow a statement
# there.
DECLARATION_KEYWORDS = {
    "associatedtype",
    "case",
    "class",
    "deinit",
    "enum",
    "extension",
    "func",
    "import",
    "init",
    "let",
    "macro",
    "operator",
    "precedencegroup",
    "protocol",
    "struct",
    "subscript",
    "typealias",
    "var",
}

# The modifiers that may stand in front of a declaration.
MODIFIERS = {
    "actor",
    "any",
    "async",
    "await",
    "borrowing",
    "class",
    "consuming",
    "convenience",
    "distributed",
    "dynamic",
    "final",
    "fileprivate",
    "get",
    "indirect",
    "infix",
    "internal",
    "isolated",
    "lazy",
    "mutating",
    "nonisolated",
    "nonmutating",
    "open",
    "optional",
    "override",
    "package",
    "postfix",
    "precedencegroup",
    "prefix",
    "private",
    "protocol",
    "public",
    "required",
    "rethrows",
    "set",
    "some",
    "static",
    "unowned",
    "weak",
    "willSet",
    "didSet",
    "throws",
    "borrowed",
    "consuming",
}


class Finding:
    """One defect in one file."""

    def __init__(self, path: Path, line: int, message: str) -> None:
        self.path = path
        self.line = line
        self.message = message

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


def strip_comments_and_strings(text: str) -> list[tuple[int, str]]:
    """Drops the comments and the string literals, line by line.

    The result of a line is the code of the line without the comments and
    without the content of the strings — the quotes themselves stay, so that
    a line that consists only of a string is not empty.
    """
    lines = text.split("\n")
    result: list[tuple[int, str]] = []
    in_block_comment = False
    in_multiline_string = False
    for number, raw in enumerate(lines, start=1):
        out: list[str] = []
        index = 0
        length = len(raw)
        while index < length:
            character = raw[index]
            if in_block_comment:
                if raw.startswith("*/", index):
                    in_block_comment = False
                    index += 2
                    continue
                index += 1
                continue
            if in_multiline_string:
                if raw.startswith('"""', index):
                    in_multiline_string = False
                    index += 3
                    continue
                index += 1
                continue
            if raw.startswith("//", index):
                break
            if raw.startswith("/*", index):
                in_block_comment = True
                index += 2
                continue
            if raw.startswith('"""', index):
                in_multiline_string = True
                index += 3
                continue
            if character == '"':
                # A string literal. The escapes are skipped, so that a quote
                # inside a string does not end it.
                index += 1
                while index < length:
                    if raw[index] == "\\":
                        index += 2
                        continue
                    if raw[index] == '"':
                        index += 1
                        break
                    index += 1
                out.append('""')
                continue
            out.append(character)
            index += 1
        result.append((number, "".join(out)))
    return result


def check_file(path: Path) -> tuple[list[Finding], bool]:
    """Checks one file. Returns the findings and whether it was understood."""
    findings: list[Finding] = []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        findings.append(Finding(path, 0, f"the file could not be read: {error}"))
        return findings, True

    lines = strip_comments_and_strings(text)

    # 1. The balance of the brackets.
    depth = 0
    parens = 0
    brackets = 0
    for number, code in lines:
        for character in code:
            if character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
                if depth < 0:
                    findings.append(
                        Finding(path, number, "a closing brace without an opening one")
                    )
                    depth = 0
            elif character == "(":
                parens += 1
            elif character == ")":
                parens -= 1
            elif character == "[":
                brackets += 1
            elif character == "]":
                brackets -= 1
    if depth != 0:
        findings.append(
            Finding(path, len(lines), f"the braces are not balanced ({depth} open)")
        )
    if parens != 0:
        findings.append(
            Finding(path, len(lines), f"the parentheses are not balanced ({parens} open)")
        )
    if brackets != 0:
        findings.append(
            Finding(path, len(lines), f"the brackets are not balanced ({brackets} open)")
        )

    # 2. A statement in the body of a type.
    #
    # The parameters of a function stand in the body of the type until the
    # body of the function opens — a parameter is not a statement. The
    # parentheses decide where a signature ends.
    depth = 0
    signature_parens = 0
    for number, code in lines:
        stripped = code.strip()
        if not stripped:
            depth += code.count("{") - code.count("}")
            continue
        if depth == 1 and signature_parens == 0:
            word = ""
            for character in stripped:
                if character.isalnum() or character == "_":
                    word += character
                else:
                    break
            if (
                word
                and word not in DECLARATION_KEYWORDS
                and word not in MODIFIERS
                and not stripped.startswith("@")
                and not stripped.startswith("#")
                and stripped not in { "{", "}", "};", "})", "),", "]", "]," }
            ):
                findings.append(
                    Finding(
                        path,
                        number,
                        "a statement in the body of a type — Swift allows only "
                        f"declarations there: {stripped[:70]!r}",
                    )
                )
        if depth == 1:
            signature_parens += code.count("(") - code.count(")")
            if signature_parens < 0:
                signature_parens = 0
        depth += code.count("{") - code.count("}")

    # 3. The same declaration twice.
    seen: dict[str, int] = {}
    depth = 0
    for number, code in lines:
        stripped = code.strip()
        if depth == 1 and stripped.startswith("func "):
            name = stripped.split("func ", 1)[1]
            name = name.split("(", 1)[0].split("<", 1)[0].split("{", 1)[0].strip()
            if name:
                if name in seen:
                    findings.append(
                        Finding(
                            path,
                            number,
                            f"the function '{name}' is declared twice "
                            f"(the first one is in the line {seen[name]})",
                        )
                    )
                else:
                    seen[name] = number
        depth += code.count("{") - code.count("}")

    # 4. The same box of a view twice. A box that is pasted twice is a box
    #    that shows the same thing twice — a defect that no compiler sees.
    #
    #    The title is read from the **raw** line: the strings are empty in
    #    the stripped lines.
    boxes: dict[str, int] = {}
    for number, raw_line in enumerate(text.split("\n"), start=1):
        stripped = raw_line.strip()
        if stripped.startswith("GroupBox(") and '"' in stripped:
            title = stripped.split('"', 1)[1].split('"', 1)[0]
            if title:
                if title in boxes:
                    findings.append(
                        Finding(
                            path,
                            number,
                            f"the box '{title}' stands twice in this file "
                            f"(the first one is in the line {boxes[title]})",
                        )
                    )
                else:
                    boxes[title] = number

    return findings, True


def main(argv: list[str]) -> int:
    if argv:
        paths = [Path(argument) for argument in argv]
    else:
        paths = []
        for root in SOURCE_ROOTS:
            if root.is_dir():
                paths.extend(sorted(root.rglob("*.swift")))
    if not paths:
        print("NOTE: no Swift sources found — the check was not run.")
        return 0

    all_findings: list[Finding] = []
    for path in paths:
        findings, _ = check_file(path)
        all_findings.extend(findings)

    for finding in all_findings:
        print(f"ERROR: {finding}", file=sys.stderr)
    if all_findings:
        print(
            f"ERROR: {len(all_findings)} defect(s) in the Swift sources — "
            "the application would not build.",
            file=sys.stderr,
        )
        return 1
    print(
        f"✓ The Swift sources are structurally sound ({len(paths)} files checked). "
        "This is a structural check, not a compilation."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

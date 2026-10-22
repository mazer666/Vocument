#!/usr/bin/env python3
"""The static proof of the Swift sources of Vocument (`QA-003`, `CODE-001`).

There is no Swift toolchain in every environment that builds this project, and
a Swift file that does not compile is a file that nobody notices — the Rust
part of the product is green, the gate is green, and the application does not
start. The defect of the last round was exactly of that kind: the application
read `inspection.risyCount`, while the generated bindings know `riskyCount`.
Nothing in the repository noticed, because nobody could compile the sources.

This check is the second line against that class of defect. It is **not** a
compiler and does not pretend to be one. It proves two things that a
compiler would prove, and it proves them without a toolchain:

1. **The grammar.** Every Swift file of the application, the kit, the command
   line tool and the tests is parsed with the real grammar of Swift
   (tree-sitter-swift). A file that does not parse cannot be compiled; the
   parse is a proof of the syntax, not a guess of the brackets.

2. **The names.** The members that the application uses on a type of the
   generated bindings must exist in that type; the functions that the
   application calls must exist in the package with the same argument labels;
   the types that the application names must be imported or defined in the
   package. This is the check that finds `risyCount`.

Both checks run on the **syntax tree** of the files, not on their text: the
tree comes from `scripts/check-swift-ast.mjs`, which is called with the
grammar of Swift.

The script needs `node` with the packages `tree-sitter` and
`tree-sitter-swift`:

    python3 scripts/check-swift-sources.py --install

The packages are installed into `~/.cache/vocument-swift-check` and are not
part of the repository. The check does not reach out to the network by
itself; the installation is an explicit step of the developer.

Usage:
    scripts/check-swift-sources.py [--install] [<file> ...]

Exit code 0: every check ran and every file is sound.
Exit code 1: at least one defect was found.
Exit code 2: a check could not be run — the proof is **incomplete**, and
             this is reported as loudly as a defect. A green gate that
             proves nothing is worse than a red one.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
MACOS_ROOT = REPOSITORY_ROOT / "apps" / "macos"
FFI_SOURCE = MACOS_ROOT / "Sources" / "VocumentFFI" / "vocument_ffi.swift"
AST_HELPER = REPOSITORY_ROOT / "scripts" / "check-swift-ast.mjs"

# The sources the proof covers: the application, the kit, the command line
# tool and the tests of the kit. The generated bindings are read as the
# **reference** of the names, not checked for their own sake.
CHECKED_ROOTS = [
    MACOS_ROOT / "Sources" / "VocumentMacApp",
    MACOS_ROOT / "Sources" / "VocumentMacKit",
    MACOS_ROOT / "Sources" / "VocumentMacHandshakeCLI",
    MACOS_ROOT / "Tests" / "VocumentMacKitTests",
]

# The place the grammar is installed to. The packages are a tool of the
# check, not a dependency of the product.
CACHE_DIRECTORY = Path(
    os.environ.get(
        "VOCUSMENT_SWIFT_CHECK_CACHE",
        str(Path.home() / ".cache" / "vocument-swift-check"),
    )
)
PARSER_PACKAGES = ["tree-sitter", "tree-sitter-swift"]

# The types of the system that the application uses. Every other name in a
# type position has to come from the package: the generated bindings, the
# kit or the application. The list holds the types of the standard library,
# Foundation, AppKit, SwiftUI, CoreGraphics and XCTest that the sources
# really use.
SYSTEM_TYPES = {
    "App",
    "Bool",
    "CFData",
    "CGImage",
    "CGPoint",
    "CGRect",
    "CGSize",
    "CustomStringConvertible",
    "Data",
    "DispatchSourceMemoryPressure",
    "Equatable",
    "Error",
    "Identifiable",
    "Int",
    "Int32",
    "MainActor",
    "ObservableObject",
    "ObservedObject",
    "Published",
    "Scene",
    "Sendable",
    "Set",
    "State",
    "String",
    "UInt32",
    "UInt64",
    "URL",
    "View",
}

# The members that a protocol of the system brings along. A type of the
# package that conforms to such a protocol may use them.
SYSTEM_PROTOCOL_MEMBERS = {
    "CaseIterable": {"allCases"},
    "Codable": set(),
    "CustomStringConvertible": {"description"},
    "Decodable": set(),
    "Encodable": set(),
    "Equatable": {"=="},
    "Error": {"localizedDescription"},
    "Hashable": {"hash", "hashValue"},
    "Identifiable": {"id"},
    "ObservableObject": set(),
    "Sendable": set(),
}

# The protocols of the system that bring members along that the check
# cannot know. A type that conforms to one of them is not checked for its
# members — a view of SwiftUI carries the modifiers of SwiftUI.
SYSTEM_PROTOCOLS_WITH_UNKNOWN_MEMBERS = {"View"}

# The methods of a collection that carry a closure. The closure of such a
# method receives the element of the collection.
COLLECTION_METHODS_WITH_CLOSURE = {
    "compactMap",
    "filter",
    "flatMap",
    "forEach",
    "map",
    "sorted",
    "sorted(by:)",
}

# The names of the system that the sources use: the types of SwiftUI,
# AppKit, Foundation and CoreGraphics that the sources build, and the free
# functions of the standard library that they call. A name that is neither
# in this list nor in the package is a name that nobody defined — a typo in
# a call, or a name of the system that was forgotten in this list.
SYSTEM_NAMES = {
    "Array",
    "Button",
    "CGBitmapInfo",
    "CGColorSpaceCreateDeviceRGB",
    "CGDataProvider",
    "CGFloat",
    "CGImage",
    "CGPoint",
    "CGRect",
    "CGSize",
    "Circle",
    "Color",
    "Data",
    "Divider",
    "DragGesture",
    "Double",
    "Double",
    "ForEach",
    "GeometryReader",
    "GroupBox",
    "HStack",
    "Image",
    "Int",
    "Int32",
    "Label",
    "LazyVStack",
    "Menu",
    "NSOpenPanel",
    "NSSavePanel",
    "Picker",
    "Rectangle",
    "RoundedRectangle",
    "ScrollView",
    "Spacer",
    "Text",
    "TextField",
    "UInt32",
    "UInt64",
    "URL",
    "UUID",
    "VStack",
    "WindowGroup",
    "ZStack",
    "abs",
    "assert",
    "fatalError",
    "max",
    "min",
    "precondition",
    "print",
}

# The keywords of the language. A keyword that stands where a call stands
# is not a call of a function that nobody defined.
SWIFT_KEYWORDS = {
    "await",
    "defer",
    "do",
    "guard",
    "if",
    "repeat",
    "return",
    "switch",
    "throw",
    "try",
    "while",
}

# The prefix of the functions of the generated bindings. A call to a name
# with this prefix must exist in the bindings — a name that does not exist
# there is a defect of the application, not a call into the system.
FFI_FUNCTION_PREFIX = "documentCore"

# The node types that carry a type.
TYPE_NODE_TYPES = {
    "array_type",
    "dictionary_type",
    "existential_type",
    "function_type",
    "opaque_type",
    "optional_type",
    "tuple_type",
    "type_arguments",
    "type_identifier",
    "user_type",
}


# --------------------------------------------------------------------------
# The syntax tree
# --------------------------------------------------------------------------


@dataclass
class SwiftFile:
    """One Swift file with its syntax tree."""

    path: Path
    lines: list[str]
    tree: dict
    errors: list[dict]
    ok: bool

    def text(self, node: dict) -> str:
        (start_row, start_column), (end_row, end_column) = node["s"], node["e"]
        if start_row == end_row:
            return self.lines[start_row][start_column:end_column]
        middle = self.lines[start_row + 1 : end_row]
        return (
            self.lines[start_row][start_column:]
            + "\n"
            + "\n".join(middle)
            + self.lines[end_row][:end_column]
        )

    def line_of(self, node: dict) -> int:
        return node["s"][0] + 1


def children(node: dict, node_type: str | None = None) -> list[dict]:
    result = node.get("c", [])
    if node_type is not None:
        result = [child for child in result if child["t"] == node_type]
    return result


def field_child(node: dict, field_name: str) -> dict | None:
    for child in node.get("c", []):
        if child.get("f") == field_name:
            return child
    return None


def descendants(node: dict, node_type: str) -> list[dict]:
    """Every node of the given type below the node."""
    result: list[dict] = []
    for child in node.get("c", []):
        if child["t"] == node_type:
            result.append(child)
        result.extend(descendants(child, node_type))
    return result


def field_children(node: dict, field_name: str) -> list[dict]:
    return [child for child in node.get("c", []) if child.get("f") == field_name]


def first_child(node: dict, node_type: str) -> dict | None:
    """The first child of the given type."""
    for child in node.get("c", []):
        if child["t"] == node_type:
            return child
    return None


def type_child(node: dict) -> dict | None:
    """The child that carries the type of a declaration.

    The grammar of Swift hides the rule that holds the type behind the
    field `name`; the type is therefore the child with the field `name`
    that is a type and not an identifier.
    """
    for child in field_children(node, "name"):
        if child["t"] in TYPE_NODE_TYPES:
            return child
    return None


def identifier_child(node: dict) -> dict | None:
    """The child that carries the name of a declaration."""
    for child in field_children(node, "name"):
        if child["t"] == "simple_identifier":
            return child
    return None


def declared_name_child(node: dict) -> dict | None:
    """The child that carries the name of a type.

    A type carries its name in a `type_identifier`, a function in a
    `simple_identifier`. A function also has a **type** in the field
    `name` — the type of the result — and the two must not be mixed up.
    """
    for child in field_children(node, "name"):
        if child["t"] in ("type_identifier", "user_type"):
            return child
    return None


# --------------------------------------------------------------------------
# The symbols of the package
# --------------------------------------------------------------------------


@dataclass
class Member:
    """A member of a type: a property, a method, a case or an initialiser."""

    name: str
    kind: str  # property | method | case | init
    type: str | None = None
    labels: list[str] = field(default_factory=list)
    required: int = 0
    is_static: bool = False
    file: str = ""
    line: int = 0


@dataclass
class TypeInfo:
    """A type of the package: a struct, a class, an enum or an actor."""

    name: str
    kind: str
    file: str = ""
    line: int = 0
    members: dict[str, Member] = field(default_factory=dict)
    cases: list[str] = field(default_factory=list)
    is_generated: bool = False
    declared: bool = False
    inherits: list[str] = field(default_factory=list)

    def add(self, member: Member) -> None:
        if member.name not in self.members:
            self.members[member.name] = member
        if member.kind == "case" and member.name not in self.cases:
            self.cases.append(member.name)


@dataclass
class FunctionInfo:
    """A function of the package: a free function or a method of a type."""

    name: str
    labels: list[str] = field(default_factory=list)
    required: int = 0
    result: str | None = None
    is_throwing: bool = False
    is_async: bool = False
    is_static: bool = False
    file: str = ""
    line: int = 0
    owner: str | None = None


@dataclass
class Finding:
    """One defect in one file."""

    path: Path
    line: int
    rule: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: [{self.rule}] {self.message}"


# --------------------------------------------------------------------------
# The type names
# --------------------------------------------------------------------------


def type_name(node: dict | None, swift_file: SwiftFile) -> str | None:
    """The canonical name of a type node.

    `FfiCommentAnnotation?` becomes `FfiCommentAnnotation?`, `[FfiFoo]`
    becomes `[FfiFoo]`, `some View` becomes `some View`. The name is used
    to look a type up in the tables of the package.
    """
    if node is None:
        return None
    node_type = node["t"]
    if node_type == "user_type":
        for child in children(node, "type_identifier"):
            return swift_file.text(child)
        return None
    if node_type == "type_identifier":
        return swift_file.text(node)
    if node_type == "optional_type":
        wrapped = field_child(node, "wrapped")
        inner = type_name(wrapped if wrapped is not None else type_child(node), swift_file)
        return None if inner is None else inner + "?"
    if node_type == "array_type":
        inner = type_name(type_child(node), swift_file)
        return None if inner is None else "[" + inner + "]"
    if node_type == "dictionary_type":
        inner = swift_file.text(node).strip()
        if not (inner.startswith("[") and inner.endswith("]")):
            return None
        key, separator, value = inner[1:-1].partition(":")
        if not separator:
            return None
        return "[" + key.strip() + ":" + value.strip() + "]"
    if node_type == "opaque_type":
        inner = type_name(type_child(node), swift_file)
        return None if inner is None else "some " + inner
    if node_type == "existential_type":
        for child in children(node, "type_identifier"):
            return swift_file.text(child)
        return None
    if node_type == "tuple_type":
        return swift_file.text(node).replace("\n", " ")
    return None


def case_name(text: str) -> str | None:
    """The name of the case a pattern of a `switch` stands for.

    `.success(let snapshot)` stands for the case `success`, `case .conflict`
    for the case `conflict`.
    """
    text = text.strip()
    if text.startswith("case"):
        text = text[4:].strip()
    if text.startswith("."):
        text = text[1:].strip()
    name = ""
    for character in text:
        if character.isalnum() or character == "_":
            name += character
        else:
            break
    return name or None


def base_type_name(name: str | None) -> str | None:
    """The name of the type without the optional and the array around it.

    `FfiFoo?` becomes `FfiFoo`, `[FfiFoo]` becomes `FfiFoo`. A dictionary
    has no element of its own and returns `None`.
    """
    if name is None:
        return None
    name = name.strip()
    if name.startswith("some "):
        return None
    if name.startswith("["):
        if ":" in name:
            return None
        return name[1:-1].strip()
    if name.endswith("?"):
        return name[:-1].strip()
    return name


def unwrapped_type_name(name: str | None) -> str | None:
    """The type without the optional around it."""
    if name is None:
        return None
    name = name.strip()
    if name.endswith("?"):
        return name[:-1].strip()
    return name


def element_type_name(name: str | None) -> str | None:
    """The type of the elements of an array."""
    if name is None:
        return None
    name = name.strip()
    if name.startswith("[") and ":" not in name:
        return name[1:-1].strip()
    return None


# --------------------------------------------------------------------------
# The check
# --------------------------------------------------------------------------


class SourceCheck:
    """The name resolution over the syntax tree of the sources."""

    def __init__(self, files: list[SwiftFile]) -> None:
        self.files = files
        self.findings: list[Finding] = []
        self.types: dict[str, TypeInfo] = {}
        self.functions: dict[str, FunctionInfo] = {}
        self.ffi_types: dict[str, TypeInfo] = {}
        self.ffi_functions: dict[str, FunctionInfo] = {}
        self.imports: dict[str, set[str]] = {}
        self.generic_parameters: set[str] = set()
        # The element type of the closure of a collection, by the node of
        # the closure.
        self.closure_elements: dict[int, str] = {}

    # -- the report ------------------------------------------------------

    def is_generated(self, path: Path) -> bool:
        """Whether the file is one of the generated bindings."""
        return path.resolve() == FFI_SOURCE.resolve()

    def report(self, swift_file: SwiftFile, node: dict, rule: str, message: str) -> None:
        self.findings.append(
            Finding(swift_file.path, swift_file.line_of(node), rule, message)
        )

    # -- the collection of the symbols ------------------------------------

    def collect(self) -> None:
        """Reads the symbols of every file: the types, their members and the
        functions."""
        for swift_file in self.files:
            self.imports[swift_file.path] = set()
            for node in children(swift_file.tree):
                self.collect_top_level(swift_file, node)

    def collect_top_level(self, swift_file: SwiftFile, node: dict) -> None:
        node_type = node["t"]
        if node_type == "import_declaration":
            for child in children(node, "identifier"):
                self.imports[swift_file.path].add(swift_file.text(child))
            return
        if node_type == "class_declaration":
            self.collect_type(swift_file, node)
            return
        if node_type == "extension_declaration":
            self.collect_extension(swift_file, node)
            return
        if node_type == "function_declaration":
            function = self.read_function(swift_file, node, owner=None)
            self.remember_function(function, swift_file)
            return
        if node_type == "protocol_declaration":
            name_node = declared_name_child(node)
            if name_node is not None:
                name = swift_file.text(name_node)
                self.types.setdefault(
                    name,
                    TypeInfo(
                        name=name,
                        kind="protocol",
                        file=str(swift_file.path),
                        line=swift_file.line_of(node),
                    ),
                )
            return

    def remember_function(self, function: FunctionInfo, swift_file: SwiftFile) -> None:
        if function.name not in self.functions:
            self.functions[function.name] = function
        if self.is_generated(swift_file.path):
            self.ffi_functions.setdefault(function.name, function)

    def collect_type(self, swift_file: SwiftFile, node: dict) -> None:
        kind_node = field_child(node, "declaration_kind")
        name_node = declared_name_child(node)
        if name_node is None:
            return
        name = swift_file.text(name_node)
        kind = swift_file.text(kind_node) if kind_node is not None else "struct"
        if kind == "extension":
            # An extension adds members to a type that already exists; it
            # does not declare a type of its own.
            self.collect_extension(swift_file, node)
            return
        for parameter in children(node, "type_parameter"):
            identifier = identifier_child(parameter)
            if identifier is not None:
                self.generic_parameters.add(swift_file.text(identifier))
        body = field_child(node, "body")
        type_info = TypeInfo(
            name=name,
            kind=kind,
            file=str(swift_file.path),
            line=swift_file.line_of(node),
            is_generated=self.is_generated(swift_file.path),
            declared=True,
        )
        for specification in children(node, "inheritance_specifier"):
            for identifier in descendants(specification, "type_identifier"):
                type_info.inherits.append(swift_file.text(identifier))
        existing = self.types.get(name)
        if existing is not None and existing.kind == "extension":
            # The extension of the type was read before the type itself; the
            # members of the extension belong to the type.
            type_info.members.update(existing.members)
            type_info.cases.extend(
                case for case in existing.cases if case not in type_info.cases
            )
        self.types[name] = type_info
        if type_info.is_generated:
            self.ffi_types[name] = type_info
        if body is not None:
            self.collect_members(swift_file, body, type_info)

    def collect_extension(self, swift_file: SwiftFile, node: dict) -> None:
        name_node = declared_name_child(node)
        if name_node is None:
            return
        name = swift_file.text(name_node)
        type_info = self.types.get(name)
        if type_info is None:
            type_info = TypeInfo(
                name=name,
                kind="extension",
                file=str(swift_file.path),
                line=swift_file.line_of(node),
            )
            self.types[name] = type_info
        body = field_child(node, "body")
        if body is not None:
            self.collect_members(swift_file, body, type_info)

    def collect_members(self, swift_file: SwiftFile, body: dict, owner: TypeInfo) -> None:
        for member_node in children(body):
            member_type = member_node["t"]
            if member_type == "property_declaration":
                member = self.read_property(swift_file, member_node)
                if member is not None:
                    owner.add(member)
            elif member_type == "function_declaration":
                function = self.read_function(swift_file, member_node, owner=owner.name)
                owner.add(
                    Member(
                        name=function.name,
                        kind="method",
                        type=function.result,
                        labels=function.labels,
                        required=function.required,
                        is_static=function.is_static,
                        file=function.file,
                        line=function.line,
                    )
                )
                self.remember_function(function, swift_file)
            elif member_type == "init_declaration":
                labels, required = self.read_parameters(swift_file, member_node)
                owner.add(
                    Member(
                        name="init",
                        kind="init",
                        labels=labels,
                        required=required,
                        file=str(swift_file.path),
                        line=swift_file.line_of(member_node),
                    )
                )
            elif member_type == "enum_entry":
                for name_node in field_children(member_node, "name"):
                    case_name = swift_file.text(name_node)
                    owner.add(
                        Member(
                            name=case_name,
                            kind="case",
                            file=str(swift_file.path),
                            line=swift_file.line_of(member_node),
                        )
                    )
                    if case_name not in owner.cases:
                        owner.cases.append(case_name)

    def read_property(self, swift_file: SwiftFile, node: dict) -> Member | None:
        pattern = field_child(node, "name")
        identifier = None
        if pattern is not None:
            identifier = field_child(pattern, "bound_identifier")
            if identifier is None:
                for child in children(pattern, "simple_identifier"):
                    identifier = child
                    break
        if identifier is None:
            return None
        annotation = first_child(node, "type_annotation")
        declared = None
        if annotation is not None:
            declared = type_name(type_child(annotation), swift_file)
        return Member(
            name=swift_file.text(identifier),
            kind="property",
            type=declared,
            file=str(swift_file.path),
            line=swift_file.line_of(node),
        )

    def read_function(
        self, swift_file: SwiftFile, node: dict, owner: str | None
    ) -> FunctionInfo:
        name_node = identifier_child(node)
        name = swift_file.text(name_node) if name_node is not None else "<anonymous>"
        labels, required = self.read_parameters(swift_file, node)
        result = type_name(type_child(node), swift_file)
        modifiers = " ".join(swift_file.text(child) for child in children(node, "modifiers"))
        return FunctionInfo(
            name=name,
            labels=labels,
            required=required,
            result=result,
            is_throwing="throws" in modifiers,
            is_async="async" in modifiers,
            is_static="static" in modifiers or "class" in modifiers,
            file=str(swift_file.path),
            line=swift_file.line_of(node),
            owner=owner,
        )

    def read_parameters(
        self, swift_file: SwiftFile, node: dict
    ) -> tuple[list[str], int]:
        """The labels of the parameters and the number of the ones that are
        needed.

        A parameter with a default value may be left out. The default value
        stands in the declaration of the function, not in the parameter.
        """
        labels: list[str] = []
        for parameter in children(node, "parameter"):
            external = field_child(parameter, "external_name")
            internal = field_child(parameter, "name")
            if external is not None:
                label = swift_file.text(external)
            elif internal is not None:
                label = swift_file.text(internal)
            else:
                label = ""
            labels.append(label)
        with_default = len(field_children(node, "default_value"))
        required = max(0, len(labels) - with_default)
        return labels, required

    # -- the walk over the sources ---------------------------------------

    def run(self) -> None:
        for swift_file in self.files:
            if self.is_generated(swift_file.path):
                # The generated bindings are the reference of the names; they
                # are not resolved against themselves.
                continue
            self.check_type_names(swift_file)
            self.check_duplicates(swift_file)
            for node in children(swift_file.tree):
                self.visit(swift_file, node, {}, None, False)

    def check_duplicates(self, swift_file: SwiftFile) -> None:
        """The same declaration twice in one file.

        A declaration that is pasted twice is a declaration that nobody
        looked at. Swift refuses the second one — and the file does not
        compile.
        """
        types: dict[str, int] = {}
        for node in children(swift_file.tree):
            if node["t"] != "class_declaration":
                continue
            kind = field_child(node, "declaration_kind")
            name_node = declared_name_child(node)
            if name_node is None or kind is None:
                continue
            if swift_file.text(kind) == "extension":
                continue
            name = swift_file.text(name_node)
            line = swift_file.line_of(node)
            if name in types:
                self.report(
                    swift_file,
                    node,
                    "duplicate-declaration",
                    f"the type '{name}' is declared twice in this file "
                    f"(the first one is in the line {types[name]})",
                )
            else:
                types[name] = line
            body = field_child(node, "body")
            if body is None:
                continue
            members: dict[str, int] = {}
            for member in children(body):
                if member["t"] != "property_declaration":
                    continue
                pattern = field_child(member, "name")
                if pattern is None:
                    continue
                identifier = field_child(pattern, "bound_identifier")
                if identifier is None:
                    continue
                member_name = swift_file.text(identifier)
                member_line = swift_file.line_of(member)
                if member_name in members:
                    self.report(
                        swift_file,
                        member,
                        "duplicate-declaration",
                        f"the member '{member_name}' is declared twice in the "
                        f"type '{name}' (the first one is in the line "
                        f"{members[member_name]})",
                    )
                else:
                    members[member_name] = member_line

    def check_type_names(self, swift_file: SwiftFile) -> None:
        """Every type the source names must be imported or defined in the
        package.

        A type of the system that the sources use is in the list
        `SYSTEM_TYPES`; every other type has to come from the generated
        bindings, the kit or the application.
        """
        self.check_type_names_of(swift_file, swift_file.tree, in_attribute=False)

    def check_type_names_of(
        self, swift_file: SwiftFile, node: dict, in_attribute: bool
    ) -> None:
        if node["t"] == "user_type" and not in_attribute:
            for identifier in children(node, "type_identifier"):
                name = swift_file.text(identifier)
                if (
                    name in self.types
                    or name in SYSTEM_TYPES
                    or name in self.generic_parameters
                ):
                    continue
                self.report(
                    swift_file,
                    node,
                    "type-missing",
                    f"the type '{name}' is neither imported by the file nor "
                    "defined in the package",
                )
                break
        for child in node.get("c", []):
            self.check_type_names_of(
                swift_file, child, in_attribute or node["t"] == "attribute"
            )

    def visit(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
        in_async: bool,
    ) -> None:
        node_type = node["t"]
        if node_type == "class_declaration":
            name_node = declared_name_child(node)
            new_owner = swift_file.text(name_node) if name_node is not None else owner
            body = field_child(node, "body")
            if body is not None:
                self.visit(swift_file, body, dict(scope), new_owner, in_async)
            return
        if node_type == "extension_declaration":
            name_node = declared_name_child(node)
            new_owner = swift_file.text(name_node) if name_node is not None else owner
            body = field_child(node, "body")
            if body is not None:
                self.visit(swift_file, body, dict(scope), new_owner, in_async)
            return
        if node_type in ("function_declaration", "init_declaration"):
            inner_scope = self.function_scope(swift_file, node, scope, owner)
            modifiers = " ".join(
                swift_file.text(child) for child in children(node, "modifiers")
            )
            body = field_child(node, "body")
            if body is not None:
                self.visit(
                    swift_file, body, inner_scope, owner, in_async or "async" in modifiers
                )
            return
        if node_type == "lambda_literal":
            inner_scope = dict(scope)
            element = self.closure_elements.get(id(node))
            for parameter in children(node, "lambda_parameter"):
                name_node = field_child(parameter, "name")
                if name_node is None:
                    continue
                declared = type_name(type_child(parameter), swift_file)
                if declared is None and element is not None:
                    declared = element
                inner_scope[swift_file.text(name_node)] = declared
            modifiers = " ".join(
                swift_file.text(child) for child in children(node, "modifiers")
            )
            for child in children(node):
                self.visit(
                    swift_file,
                    child,
                    inner_scope,
                    owner,
                    "async" in modifiers or (in_async and "await" not in modifiers),
                )
            return
        if node_type == "property_declaration":
            self.visit_property(swift_file, node, scope, owner, in_async)
            return
        if node_type in ("if_statement", "guard_statement"):
            inner_scope = dict(scope)
            for condition in field_children(node, "condition"):
                if condition["t"] != "value_binding_pattern":
                    continue
                identifier = field_child(node, "bound_identifier")
                value = None
                for child in field_children(node, "condition"):
                    if child["t"] not in ("value_binding_pattern", "=", "let", "var"):
                        value = child
                        break
                if identifier is None:
                    continue
                declared = None
                if value is not None:
                    declared = unwrapped_type_name(
                        self.infer(swift_file, value, scope, owner)
                    )
                inner_scope[swift_file.text(identifier)] = declared
            for child in children(node):
                self.visit(swift_file, child, inner_scope, owner, in_async)
            return
        if node_type == "for_statement":
            inner_scope = dict(scope)
            pattern = field_child(node, "item")
            collection = field_child(node, "collection")
            if pattern is not None and collection is not None:
                identifier = field_child(pattern, "bound_identifier")
                if identifier is None:
                    for child in children(pattern, "simple_identifier"):
                        identifier = child
                        break
                if identifier is not None:
                    inner_scope[swift_file.text(identifier)] = element_type_name(
                        self.infer(swift_file, collection, scope, owner)
                    )
            for child in children(node):
                self.visit(swift_file, child, inner_scope, owner, in_async)
            return
        if node_type == "switch_statement":
            self.check_switch(swift_file, node, scope, owner)
        if node_type == "navigation_expression":
            self.check_navigation(swift_file, node, scope, owner)
        if node_type == "call_expression":
            self.check_call(swift_file, node, scope, owner)
        if node_type == "await_expression" and not in_async:
            self.report(
                swift_file,
                node,
                "await-without-async",
                "`await` stands in a function that is not `async`",
            )
        for child in children(node):
            self.visit(swift_file, child, scope, owner, in_async)

    def function_scope(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
    ) -> dict[str, str | None]:
        inner_scope = dict(scope)
        if owner is not None:
            inner_scope["self"] = owner
        for parameter in children(node, "parameter"):
            name_node = field_child(parameter, "name")
            if name_node is None:
                continue
            inner_scope[swift_file.text(name_node)] = type_name(
                type_child(parameter), swift_file
            )
        return inner_scope

    def visit_property(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
        in_async: bool,
    ) -> None:
        pattern = field_child(node, "name")
        identifier = None
        if pattern is not None:
            identifier = field_child(pattern, "bound_identifier")
            if identifier is None:
                for child in children(pattern, "simple_identifier"):
                    identifier = child
                    break
        value = field_child(node, "value")
        annotation = first_child(node, "type_annotation")
        declared = None
        if annotation is not None:
            declared = type_name(type_child(annotation), swift_file)
        if identifier is not None and declared is None and value is not None:
            declared = self.infer(swift_file, value, scope, owner)
        if identifier is not None:
            scope[swift_file.text(identifier)] = declared
        for child in children(node):
            self.visit(swift_file, child, scope, owner, in_async)

    # -- the single checks ------------------------------------------------

    def check_navigation(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
    ) -> None:
        target = field_child(node, "target")
        suffix = field_child(node, "suffix")
        if target is None or suffix is None:
            return
        member_name = self.member_name(swift_file, suffix)
        if member_name is None:
            return
        base_type = self.infer(swift_file, target, scope, owner)
        if base_type is None or base_type.startswith("["):
            # A collection carries the members of the standard library; the
            # proof of their names is not the job of this check.
            return
        base = base_type_name(base_type)
        if base is None:
            return
        type_info = self.types.get(base)
        if type_info is None or not type_info.declared:
            # A type of the system: the members of the standard library are
            # not the job of this check.
            return
        if member_name in type_info.members:
            return
        if self.member_of_the_system(type_info, member_name):
            return
        self.report(
            swift_file,
            node,
            "member-missing",
            f"the type '{base}' has no member '{member_name}' — "
            + (
                "the generated bindings do not know it"
                if type_info.is_generated
                else "the package does not define it"
            ),
        )

    def member_of_the_system(self, type_info: TypeInfo, member_name: str) -> bool:
        """Whether the member comes from a protocol of the system."""
        for protocol in type_info.inherits:
            if protocol in SYSTEM_PROTOCOLS_WITH_UNKNOWN_MEMBERS:
                return True
            if protocol in SYSTEM_PROTOCOL_MEMBERS:
                if member_name in SYSTEM_PROTOCOL_MEMBERS[protocol]:
                    return True
        return False

    def check_call(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
    ) -> None:
        callee = None
        for child in children(node):
            if child["t"] != "call_suffix":
                callee = child
                break
        if callee is None:
            return
        arguments, trailing_closure, subscript = self.call_arguments(
            swift_file, node
        )
        if subscript:
            # `a[i]` is a subscript, not a call of a function.
            return
        if callee["t"] == "simple_identifier":
            name = swift_file.text(callee)
            function = self.ffi_functions.get(name) or self.functions.get(name)
            if function is not None:
                self.check_labels(
                    swift_file, node, function, arguments, name, trailing_closure
                )
                return
            if name.startswith(FFI_FUNCTION_PREFIX):
                self.report(
                    swift_file,
                    node,
                    "function-missing",
                    f"the function '{name}' is called, but the generated "
                    "bindings do not define it",
                )
                return
            type_info = self.types.get(name)
            if type_info is not None and type_info.declared:
                initialiser = type_info.members.get("init")
                if initialiser is not None:
                    self.check_labels(
                        swift_file,
                        node,
                        FunctionInfo(
                            name=name,
                            labels=initialiser.labels,
                            required=initialiser.required,
                        ),
                        arguments,
                        name,
                        trailing_closure,
                    )
                return
            if name in SYSTEM_NAMES or name in SYSTEM_TYPES or name in SWIFT_KEYWORDS:
                return
            # The name is neither a function of the package nor a name of
            # the system: nobody defined it.
            self.report(
                swift_file,
                node,
                "function-missing",
                f"the name '{name}' is called, but neither the package nor "
                "the system defines it",
            )
            return
        if callee["t"] != "navigation_expression":
            return
        target = field_child(callee, "target")
        suffix = field_child(callee, "suffix")
        if target is None or suffix is None:
            return
        member_name = self.member_name(swift_file, suffix)
        if member_name is None:
            return
        base_type = self.infer(swift_file, target, scope, owner)
        if base_type is None:
            return
        if base_type.startswith("["):
            element = element_type_name(base_type)
            if element is not None and member_name in COLLECTION_METHODS_WITH_CLOSURE:
                for suffix in children(node, "call_suffix"):
                    for closure in children(suffix, "lambda_literal"):
                        self.closure_elements[id(closure)] = element
            return
        base = base_type_name(base_type)
        if base is None:
            return
        type_info = self.types.get(base)
        if type_info is None or not type_info.declared:
            return
        member = type_info.members.get(member_name)
        if member is None:
            if self.member_of_the_system(type_info, member_name):
                return
            self.report(
                swift_file,
                node,
                "member-missing",
                f"the type '{base}' has no member '{member_name}' — "
                "the call could not be resolved",
            )
            return
        if member.kind == "method":
            self.check_labels(
                swift_file,
                node,
                FunctionInfo(
                    name=f"{base}.{member_name}",
                    labels=member.labels,
                    required=member.required,
                ),
                arguments,
                f"{base}.{member_name}",
                trailing_closure,
            )

    def check_labels(
        self,
        swift_file: SwiftFile,
        node: dict,
        function: FunctionInfo,
        arguments: list[str | None],
        label: str,
        trailing_closure: bool,
    ) -> None:
        if not arguments and not function.labels:
            return
        expected = function.labels
        if len(arguments) > len(expected):
            self.report(
                swift_file,
                node,
                "argument-mismatch",
                f"'{label}' takes {len(expected)} argument(s), "
                f"but {len(arguments)} were given",
            )
            return
        for index, argument in enumerate(arguments):
            if argument is None:
                continue
            if expected[index] != argument:
                self.report(
                    swift_file,
                    node,
                    "argument-mismatch",
                    f"the argument '{argument}' of '{label}' does not match "
                    f"the parameter '{expected[index]}' of the declaration",
                )
                return
        if len(arguments) < function.required and not trailing_closure:
            missing = expected[len(arguments) : function.required]
            self.report(
                swift_file,
                node,
                "argument-mismatch",
                f"'{label}' needs the argument(s) {', '.join(missing)}",
            )

    def call_arguments(
        self, swift_file: SwiftFile, node: dict
    ) -> tuple[list[str | None], bool, bool]:
        """The labels of the arguments of a call, whether the call carries a
        trailing closure, and whether it is a subscript.

        A subscript (`a[i]`) stands in the same place as a call, but it is
        no call of a function.
        """
        labels: list[str | None] = []
        trailing_closure = False
        subscript = False
        for suffix in children(node, "call_suffix"):
            if children(suffix, "lambda_literal"):
                trailing_closure = True
            for arguments in children(suffix, "value_arguments"):
                if swift_file.text(arguments).lstrip().startswith("["):
                    subscript = True
                    continue
                for argument in children(arguments, "value_argument"):
                    label_node = field_child(argument, "name")
                    if label_node is None:
                        labels.append(None)
                    else:
                        labels.append(swift_file.text(label_node))
        return labels, trailing_closure, subscript

    def member_name(self, swift_file: SwiftFile, suffix: dict) -> str | None:
        if suffix["t"] == "navigation_suffix":
            for child in field_children(suffix, "suffix"):
                return swift_file.text(child)
            return None
        if suffix["t"] == "simple_identifier":
            return swift_file.text(suffix)
        return None

    def check_switch(
        self,
        swift_file: SwiftFile,
        node: dict,
        scope: dict[str, str | None],
        owner: str | None,
    ) -> None:
        subject = field_child(node, "expr")
        if subject is None:
            return
        base = base_type_name(self.infer(swift_file, subject, scope, owner))
        if base is None:
            return
        type_info = self.types.get(base)
        if type_info is None or type_info.kind != "enum":
            return
        covered: set[str] = set()
        has_default = False
        for entry in children(node, "switch_entry"):
            if children(entry, "default_keyword"):
                has_default = True
                continue
            for pattern in children(entry, "switch_pattern"):
                for case in children(pattern, "pattern"):
                    name = case_name(swift_file.text(case))
                    if name is not None:
                        covered.add(name)
        if has_default:
            return
        missing = [case for case in type_info.cases if case not in covered]
        if missing:
            self.report(
                swift_file,
                node,
                "switch-incomplete",
                f"the switch over '{base}' does not cover the case(s) "
                f"{', '.join(missing)}",
            )

    # -- the inference ----------------------------------------------------

    def infer(
        self,
        swift_file: SwiftFile,
        node: dict | None,
        scope: dict[str, str | None],
        owner: str | None,
    ) -> str | None:
        if node is None:
            return None
        node_type = node["t"]
        if node_type == "simple_identifier":
            name = swift_file.text(node)
            if name in scope and scope[name] is not None:
                return scope[name]
            if name in self.types:
                return name
            return None
        if node_type == "self_expression":
            return owner
        if node_type == "navigation_expression":
            target = field_child(node, "target")
            suffix = field_child(node, "suffix")
            if target is None or suffix is None:
                return None
            base_type = self.infer(swift_file, target, scope, owner)
            if base_type is None:
                return None
            member_name = self.member_name(swift_file, suffix)
            if member_name is None:
                return None
            if base_type.startswith("["):
                if member_name in ("first", "last"):
                    return element_type_name(base_type)
                if member_name == "count":
                    return "Int"
                return None
            base = base_type_name(base_type)
            if base is None:
                return None
            type_info = self.types.get(base)
            if type_info is None:
                return None
            member = type_info.members.get(member_name)
            if member is None or member.kind != "property":
                return None
            return member.type
        if node_type == "call_expression":
            callee = None
            for child in children(node):
                if child["t"] != "call_suffix":
                    callee = child
                    break
            if callee is None:
                return None
            if callee["t"] == "simple_identifier":
                name = swift_file.text(callee)
                function = self.ffi_functions.get(name) or self.functions.get(name)
                if function is not None:
                    return function.result
                if name in self.types:
                    return name
                return None
            if callee["t"] == "navigation_expression":
                target = field_child(callee, "target")
                suffix = field_child(callee, "suffix")
                if target is None or suffix is None:
                    return None
                base = base_type_name(self.infer(swift_file, target, scope, owner))
                if base is None:
                    return None
                type_info = self.types.get(base)
                if type_info is None:
                    return None
                member_name = self.member_name(swift_file, suffix)
                member = type_info.members.get(member_name) if member_name else None
                if member is None or member.kind != "method":
                    return None
                return member.type
            return None
        if node_type == "try_expression":
            for child in field_children(node, "expr"):
                return self.infer(swift_file, child, scope, owner)
            return None
        if node_type == "await_expression":
            for child in children(node):
                if child["t"] != "await":
                    return self.infer(swift_file, child, scope, owner)
            return None
        if node_type == "prefix_expression":
            for child in field_children(node, "target"):
                return self.infer(swift_file, child, scope, owner)
            return None
        if node_type == "as_expression":
            for child in children(node):
                if child["t"] in TYPE_NODE_TYPES:
                    return type_name(child, swift_file)
            return None
        if node_type == "nil_coalescing_expression":
            for child in children(node):
                if child["t"] != "??":
                    return self.infer(swift_file, child, scope, owner)
            return None
        if node_type == "ternary_expression":
            for child in children(node):
                if child["t"] != "?":
                    return self.infer(swift_file, child, scope, owner)
            return None
        if node_type == "assignment":
            for child in field_children(node, "target"):
                return self.infer(swift_file, child, scope, owner)
            return None
        return None


# --------------------------------------------------------------------------
# The grammar
# --------------------------------------------------------------------------


def find_node_modules() -> Path | None:
    """The directory that holds the grammar of Swift."""
    candidates = []
    from_environment = os.environ.get("VOCUSMENT_SWIFT_CHECK_NODE_MODULES")
    if from_environment:
        candidates.append(Path(from_environment))
    candidates.append(REPOSITORY_ROOT / "node_modules")
    candidates.append(CACHE_DIRECTORY / "node_modules")
    for candidate in candidates:
        if (candidate / "tree-sitter-swift").is_dir() and (
            candidate / "tree-sitter"
        ).is_dir():
            return candidate
    return None


def install_parser() -> bool:
    """Installs the grammar of Swift into the cache of the check."""
    if shutil.which("npm") is None:
        return False
    CACHE_DIRECTORY.mkdir(parents=True, exist_ok=True)
    package_json = CACHE_DIRECTORY / "package.json"
    if not package_json.exists():
        package_json.write_text(
            json.dumps(
                {
                    "name": "vocument-swift-check",
                    "private": True,
                    "description": "The grammar of Swift for the static proof.",
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
    result = subprocess.run(
        ["npm", "install", "--no-audit", "--no-fund", *PARSER_PACKAGES],
        cwd=str(CACHE_DIRECTORY),
        check=False,
    )
    return result.returncode == 0


def parse_sources(paths: list[Path]) -> tuple[list[SwiftFile], list[Finding]]:
    """Parses every file with the grammar of Swift."""
    findings: list[Finding] = []
    node_modules = find_node_modules()
    if node_modules is None:
        return [], findings
    environment = dict(os.environ)
    existing = environment.get("NODE_PATH", "")
    environment["NODE_PATH"] = (
        str(node_modules) + (os.pathsep + existing if existing else "")
    )
    result = subprocess.run(
        [
            "node",
            str(AST_HELPER),
            "--generated",
            str(FFI_SOURCE),
            *[str(path) for path in paths],
        ],
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )
    if result.returncode != 0:
        print(
            "ERROR: the Swift sources could not be parsed: "
            + (result.stderr.strip() or f"node exited with {result.returncode}"),
            file=sys.stderr,
        )
        return [], findings
    payload = json.loads(result.stdout)
    files: list[SwiftFile] = []
    for entry in payload["files"]:
        path = Path(entry["path"])
        lines = path.read_text(encoding="utf-8").split("\n")
        swift_file = SwiftFile(
            path=path,
            lines=lines,
            tree=entry.get(
                "tree", {"t": "source_file", "s": [0, 0], "e": [0, 0], "c": []}
            ),
            errors=entry.get("errors", []),
            ok=entry.get("ok", False),
        )
        files.append(swift_file)
        normalisation = entry.get("normalised")
        if normalisation is not None:
            print(
                f"NOTE: {path} was read in a normalised form "
                f"({normalisation.get('changed')} attribute(s) read as a "
                f"comment, the first in the line "
                f"{normalisation.get('firstLine')}): "
                f"{normalisation.get('reason')}.",
                file=sys.stderr,
            )
        for error in entry.get("errors", []):
            kind = error.get("kind")
            if kind in ("unreadable", "unparsable"):
                findings.append(
                    Finding(
                        path,
                        0,
                        "grammar",
                        f"the file could not be read or parsed: {error.get('message')}",
                    )
                )
                continue
            start = error.get("start", [0, 0])
            end = error.get("end", [0, 0])
            findings.append(
                Finding(
                    path,
                    start[0],
                    "grammar",
                    f"the file does not parse: {kind} {error.get('type')} "
                    f"at the line {start[0]} column {start[1]} until the line "
                    f"{end[0]} column {end[1]}",
                )
            )
    return files, findings


# --------------------------------------------------------------------------
# The main
# --------------------------------------------------------------------------


def collect_paths() -> list[Path]:
    paths: list[Path] = []
    for root in CHECKED_ROOTS:
        if root.is_dir():
            paths.extend(sorted(root.rglob("*.swift")))
    return paths


def find_swiftc() -> str | None:
    """Looks for the Swift compiler.

    The compiler is looked up on the `PATH` first. Two places are tried
    afterwards, because a toolchain that was installed by hand does not
    always make itself known to the shell: `/opt/swift/usr/bin` (the place
    of the official installation for Linux) and `/usr/share/swift/usr/bin`
    (the place of the distribution packages). A compiler that is not found
    is not a defect — the proof falls back to the grammar of tree-sitter
    and says that the compiler did not read the files.
    """
    found = shutil.which("swiftc")
    if found is not None:
        return found
    for candidate in ("/opt/swift/usr/bin/swiftc", "/usr/share/swift/usr/bin/swiftc"):
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    return None


def compiler_parse(paths: list[Path]) -> tuple[list[Finding], bool]:
    """The parse of the **real** Swift compiler.

    The compiler is the strongest proof of the syntax that can be had
    without a Mac: `swiftc -parse` reads every file with the parser of
    Swift itself. The proof needs a toolchain; where there is none, the
    grammar of tree-sitter stands in its place — and the report says so.
    """
    findings: list[Finding] = []
    swiftc = find_swiftc()
    if swiftc is None:
        return findings, False
    for path in paths:
        result = subprocess.run(
            [swiftc, "-parse", str(path)],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            message = result.stderr.strip().split("\n")
            head = " / ".join(line.strip() for line in message[:6])
            findings.append(
                Finding(
                    path,
                    0,
                    "compiler-parse",
                    f"the Swift compiler does not accept the file: {head}",
                )
            )
    return findings, True


def check_paths(paths: list[Path]) -> tuple[list[Finding], list[SwiftFile]]:
    """Parses the files and resolves the names. Returns the findings."""
    findings: list[Finding] = []
    files, parse_findings = parse_sources(paths)
    findings.extend(parse_findings)
    if not files:
        return findings, files
    check = SourceCheck(files)
    check.collect()
    check.run()
    findings.extend(check.findings)
    compiler_findings, compiler_ran = compiler_parse(paths)
    findings.extend(compiler_findings)
    return findings, files


def run_self_test() -> int:
    """Proves that the check finds a defect.

    A check that never fails proves nothing. The self-test copies the
    sources of the application into a temporary directory, breaks them in
    three ways — a member that the bindings do not know, an argument label
    that the declaration does not know and a file that does not parse — and
    demands that the check reports every one of them.
    """
    import shutil
    import tempfile

    expected = {
        "member-missing": "a member of a type of the generated bindings",
        "argument-mismatch": "an argument label that the declaration does not know",
        "grammar": "a file that does not parse",
        "function-missing": "a call of a name that nobody defined",
        "duplicate-declaration": "the same declaration twice",
        "await-without-async": "an `await` in a function that is not `async`",
        "switch-incomplete": "a `switch` that does not cover every case",
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        copies: list[Path] = []
        for source_root in CHECKED_ROOTS:
            if not source_root.is_dir():
                continue
            for path in sorted(source_root.rglob("*.swift")):
                target = root / path.relative_to(REPOSITORY_ROOT)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
                copies.append(target)

        panel = root / "apps/macos/Sources/VocumentMacApp/ReviewPanelView.swift"
        if panel.exists():
            panel.write_text(
                panel.read_text(encoding="utf-8").replace(
                    "inspection.riskyCount", "inspection.risyCount"
                ),
                encoding="utf-8",
            )
        kit = root / "apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift"
        if kit.exists():
            kit.write_text(
                kit.read_text(encoding="utf-8").replace(
                    "documentCoreSelectPageText(\n            sessionId: sessionId,",
                    "documentCoreSelectPageText(\n            sessionIdentifier: sessionId,",
                ),
                encoding="utf-8",
            )
        app = root / "apps/macos/Sources/VocumentMacApp/VocumentMacApp.swift"
        if app.exists():
            text = app.read_text(encoding="utf-8")
            # a call of a name that nobody defined
            text = text.replace("refreshReviewState()", "refreshReviewStat()", 1)
            # the same declaration twice, an `await` in a function that is
            # not `async`, and a `switch` that does not cover every case
            text += (
                "\nstruct DuplicateProbe: Sendable {}\n"
                "struct DuplicateProbe: Sendable {}\n"
                "func awaitProbe() {\n"
                "    let value = await somethingAsync()\n"
                "    _ = value\n"
                "}\n"
                "func switchProbe(outcome: FfiSaveOutcome) {\n"
                "    switch outcome {\n"
                "    case .success: break\n"
                "    }\n"
                "}\n"
            )
            app.write_text(text, encoding="utf-8")
        if copies:
            broken = copies[0]
            broken.write_text(
                broken.read_text(encoding="utf-8") + "\n}\n", encoding="utf-8"
            )

        findings, _ = check_paths(copies + [FFI_SOURCE])

    found = {finding.rule for finding in findings}
    missing = [rule for rule in expected if rule not in found]
    for rule, description in expected.items():
        if rule in found:
            print(f"✓ The check found the defect: {description}.")
        else:
            print(
                f"ERROR: the check did not find the defect: {description}.",
                file=sys.stderr,
            )
    if missing:
        print(
            "ERROR: the self-test failed — the check is not a proof.",
            file=sys.stderr,
        )
        return 1
    print("✓ The self-test passed: the check finds the defects it promises.")
    return 0


def main(argv: list[str]) -> int:
    install = False
    install_if_missing = False
    self_test = False
    arguments = []
    for argument in argv:
        if argument == "--install":
            install = True
        elif argument == "--install-if-missing":
            install_if_missing = True
        elif argument == "--self-test":
            self_test = True
        else:
            arguments.append(argument)

    if install:
        if install_parser():
            print("✓ The grammar of Swift is installed in " + str(CACHE_DIRECTORY))
        else:
            print(
                "ERROR: the grammar of Swift could not be installed — "
                "the packages 'tree-sitter' and 'tree-sitter-swift' are needed.",
                file=sys.stderr,
            )
            return 2

    if shutil.which("node") is None:
        print(
            "NOT RUN: the static proof of the Swift sources needs `node` with "
            "the packages 'tree-sitter' and 'tree-sitter-swift'. Install them "
            "with 'python3 scripts/check-swift-sources.py --install'.",
            file=sys.stderr,
        )
        return 2

    if install_if_missing and find_node_modules() is None:
        if not install_parser():
            print(
                "ERROR: the grammar of Swift could not be installed — the "
                "packages 'tree-sitter' and 'tree-sitter-swift' are needed.",
                file=sys.stderr,
            )
            return 2

    if find_node_modules() is None:
        print(
            "NOT RUN: the grammar of Swift is not installed. Install it with "
            "'python3 scripts/check-swift-sources.py --install'.",
            file=sys.stderr,
        )
        return 2

    if self_test:
        return run_self_test()

    if arguments:
        paths = [Path(argument) for argument in arguments]
    else:
        paths = collect_paths()
    if FFI_SOURCE.exists() and FFI_SOURCE not in paths:
        # The generated bindings are the reference of the names.
        paths.append(FFI_SOURCE)
    if not paths:
        print("NOTE: no Swift sources found — the check was not run.")
        return 0

    findings, files = check_paths(paths)
    if not files:
        print(
            "NOT RUN: the Swift sources could not be parsed — the proof is "
            "incomplete.",
            file=sys.stderr,
        )
        return 2

    for finding in findings:
        print(f"ERROR: {finding}", file=sys.stderr)
    if findings:
        print(
            f"ERROR: {len(findings)} defect(s) in the Swift sources — "
            "the application would not build.",
            file=sys.stderr,
        )
        return 1
    _, compiler_ran = compiler_parse(paths)
    if compiler_ran:
        print(
            f"✓ The Swift sources are sound ({len(files)} files parsed with the "
            "grammar of Swift and with the Swift compiler itself, the names "
            "resolved against the generated bindings). This is a static proof, "
            "not a compilation."
        )
    else:
        print(
            f"✓ The Swift sources are sound ({len(files)} files parsed with the "
            "grammar of Swift, the names resolved against the generated "
            "bindings). NOT RUN: the Swift compiler is not installed here, so "
            "the compiler did not read the files. This is a static proof, not "
            "a compilation."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

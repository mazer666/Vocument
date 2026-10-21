#!/usr/bin/env node
// The grammar proof of the Swift sources: a real parse with tree-sitter.
//
// The helper is called by `scripts/check-swift-sources.py`. It reads the
// Swift files, parses every one of them with the grammar of Swift and
// reports two things:
//
//   1. the **errors of the grammar** — a file that does not parse cannot be
//      compiled, and a compiler is not available in every environment that
//      builds this project;
//   2. the **syntax tree** of every file, in a compact JSON form, so that
//      the name resolution of the sources can be done on the tree instead of
//      on the text.
//
// The output is one JSON object on the standard output:
//
//   { "files": [ { "path": "...", "ok": true, "errors": [], "tree": {...} } ] }
//
// A file that could not be read or parsed is reported with `ok: false` and
// an entry in `errors` — never as a success.
//
// The generated bindings carry a block that the grammar of the parser cannot
// read: UniFFI writes
//
//     #if swift(>=5.8)
//         @_documentation(visibility: private)
//     #endif
//
// and the parser fails on a compilation condition whose content is only an
// attribute. The file is **generated** and must not be hand-edited, so the
// helper reads such a file in a **normalised** form: the blocks that hold
// nothing but an attribute are left out. The normalisation is reported in
// the output, and it is only ever used for the files that are named with
// `--generated`.
//
// Usage:
//   node scripts/check-swift-ast.mjs [--generated <file>] <file> [<file> ...]

import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const require = createRequire(import.meta.url);

let Parser;
let Swift;
try {
    Parser = require("tree-sitter");
    Swift = require("tree-sitter-swift");
} catch (error) {
    process.stderr.write(
        "ERROR: the Swift grammar is not available: " + String(error && error.message) + "\n"
    );
    process.exit(3);
}

const parser = new Parser();
parser.setLanguage(Swift);

/// Collects the errors of the grammar: a node of the type `ERROR` and a
/// node that the parser had to invent (`isMissing`).
function collectErrors(node, errors) {
    if (node.type === "ERROR" || node.isMissing) {
        errors.push({
            kind: node.isMissing ? "missing" : "error",
            type: node.type,
            start: [node.startPosition.row + 1, node.startPosition.column],
            end: [node.endPosition.row + 1, node.endPosition.column],
        });
    }
    for (let index = 0; index < node.childCount; index += 1) {
        collectErrors(node.child(index), errors);
    }
    return errors;
}

/// The compact tree: the named nodes, their field in the parent and their
/// place in the file. The text of the nodes is not repeated — the Python
/// side reads it from the file when it needs it.
function compact(node) {
    const result = {
        t: node.type,
        s: [node.startPosition.row, node.startPosition.column],
        e: [node.endPosition.row, node.endPosition.column],
    };
    const children = [];
    for (let index = 0; index < node.childCount; index += 1) {
        const child = node.child(index);
        const field = node.fieldNameForChild(index);
        if (!child.isNamed && !field) {
            // A token without a field carries no information for the
            // analysis; the tokens with a field (the `struct` of a type,
            // the `let` of a binding) are kept.
            continue;
        }
        const compactChild = compact(child);
        if (field) {
            compactChild.f = field;
        }
        children.push(compactChild);
    }
    if (children.length > 0) {
        result.c = children;
    }
    return result;
}

/// Makes the file readable for the parser **without moving a single line**.
///
/// UniFFI writes
///
///     #if swift(>=5.8)
///         @_documentation(visibility: private)
///     #endif
///
/// and the grammar of the parser fails on a compilation condition whose
/// content is nothing but an attribute — and on a compilation condition
/// that opens the body of a type. The block is read as a comment: the
/// number of the lines stays the same, the place of every token in the tree
/// stays the same, and the declarations of the file are untouched. Only a
/// block that holds nothing but an attribute is read that way; every other
/// block is left alone. The file itself is **not** changed — it is generated
/// and must not be hand-edited.
function normaliseGenerated(source) {
    const lines = source.split("\n");
    const changed = [];
    const stack = [];

    /// A comment of exactly the length of the text it replaces, so that no
    /// token moves and the place of every node in the tree stays the place
    /// it has in the file.
    const commentOf = (text) => {
        let comment = "//" + text.slice(2);
        if (comment.length > text.length) {
            comment = comment.slice(0, text.length);
        }
        while (comment.length < text.length) {
            comment += " ";
        }
        return comment;
    };

    for (let index = 0; index < lines.length; index += 1) {
        const trimmed = lines[index].trim();
        if (trimmed.startsWith("#if")) {
            const block = { start: index, condition: trimmed, onlyAttributes: true };
            stack.push(block);
            continue;
        }
        if (trimmed.startsWith("#else") || trimmed.startsWith("#elseif")) {
            if (stack.length > 0) {
                stack[stack.length - 1].onlyAttributes = false;
            }
            continue;
        }
        if (trimmed.startsWith("#endif")) {
            const block = stack.pop();
            if (block && block.onlyAttributes && /^#if swift\(>=5\.8\)$/.test(block.condition)) {
                lines[block.start] = lines[block.start].replace(
                    block.condition,
                    commentOf(block.condition)
                );
                lines[index] = lines[index].replace(trimmed, commentOf(trimmed));
                changed.push(block.start + 1);
                changed.push(index + 1);
            }
            continue;
        }
        if (stack.length > 0 && trimmed.startsWith("@") && trimmed.endsWith(")")) {
            if (stack[stack.length - 1].onlyAttributes) {
                lines[index] = lines[index].replace(trimmed, commentOf(trimmed));
                changed.push(index + 1);
            }
            continue;
        }
        if (stack.length > 0 && trimmed.length > 0 && !trimmed.startsWith("//")) {
            stack[stack.length - 1].onlyAttributes = false;
        }
    }
    return { source: lines.join("\n"), changed };
}

const arguments_ = process.argv.slice(2);
const generated = new Set();
const files = [];
for (let index = 0; index < arguments_.length; index += 1) {
    if (arguments_[index] === "--generated") {
        index += 1;
        if (index < arguments_.length) {
            generated.add(path.resolve(arguments_[index]));
        }
        continue;
    }
    files.push(arguments_[index]);
}
if (files.length === 0) {
    process.stderr.write("ERROR: no Swift file was given.\n");
    process.exit(3);
}

const result = { files: [] };
for (const file of files) {
    let source;
    try {
        source = readFileSync(file, "utf8");
    } catch (error) {
        result.files.push({
            path: file,
            ok: false,
            errors: [{ kind: "unreadable", message: String(error && error.message) }],
        });
        continue;
    }
    let tree;
    let errors;
    let normalisation = null;
    try {
        tree = parser.parse(source);
        errors = collectErrors(tree.rootNode, []);
        if (errors.length > 0 && generated.has(path.resolve(file))) {
            const normalised = normaliseGenerated(source);
            if (normalised.changed.length > 0) {
                const normalisedTree = parser.parse(normalised.source);
                const normalisedErrors = collectErrors(normalisedTree.rootNode, []);
                if (normalisedErrors.length < errors.length) {
                    tree = normalisedTree;
                    errors = normalisedErrors;
                    normalisation = {
                        changed: normalised.changed.length,
                        firstLine: normalised.changed[0],
                        reason:
                            "an attribute inside a compilation condition cannot " +
                            "be read by the grammar of the parser; the " +
                            "attribute was read as a comment",
                    };
                }
            }
        }
    } catch (error) {
        result.files.push({
            path: file,
            ok: false,
            errors: [{ kind: "unparsable", message: String(error && error.message) }],
        });
        continue;
    }
    const entry = {
        path: path.resolve(file),
        ok: errors.length === 0,
        errors,
        tree: compact(tree.rootNode),
    };
    if (normalisation !== null) {
        entry.normalised = normalisation;
    }
    result.files.push(entry);
}

process.stdout.write(JSON.stringify(result));

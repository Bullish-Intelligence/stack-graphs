#!/usr/bin/env python3
"""Generate a Python stub for the stack graphs rules from a typeshed stub.

usage: gen_stubs.py MODULE PYI TYPESHED_COMMIT OUT_PY

MODULE is "builtins" or "typing". PYI is the typeshed file stdlib/MODULE.pyi.
The script keeps names and class structure and drops all typing syntax.
Source: typeshed (Apache-2.0 and MIT).
"""
import ast
import re
import sys

module, pyi_path, commit, out_path = sys.argv[1:5]
assert module in ("builtins", "typing"), module
tree = ast.parse(open(pyi_path, encoding="utf-8").read())

# Typeshed declares these names as `_Alias()` or `_SpecialForm`, with no target.
# At run time each one stands for a class. Map it to the builtin class.
TYPING_ALIASES = {
    "List": "list",
    "Dict": "dict",
    "Set": "set",
    "FrozenSet": "frozenset",
    "Tuple": "tuple",
    "Type": "type",
    "Text": "str",
}

KEEP_DECORATORS = {"staticmethod", "classmethod", "property"}
# Module globals that Python defines for every module.
MODULE_CONSTANTS = [
    "__name__", "__file__", "__doc__", "__package__", "__spec__",
    "__loader__", "__builtins__", "__debug__",
]


def flatten(body):
    """Yield statements. Take both branches of every `if` (version and platform checks)."""
    for stmt in body:
        if isinstance(stmt, ast.If):
            yield from flatten(stmt.body)
            yield from flatten(stmt.orelse)
        else:
            yield stmt


def public(name):
    return not name.startswith("_") or (name.startswith("__") and name.endswith("__"))


# Pass 1: collect the public top-level names the stub defines.
top_names = set()
for stmt in flatten(tree.body):
    if isinstance(stmt, (ast.ClassDef, ast.FunctionDef)) and public(stmt.name):
        top_names.add(stmt.name)
    elif isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name) and public(stmt.target.id):
        top_names.add(stmt.target.id)
    elif isinstance(stmt, ast.Assign):
        for t in stmt.targets:
            if isinstance(t, ast.Name) and public(t.id):
                top_names.add(t.id)


# A typing class can derive from a builtin class, as NamedTuple derives from tuple.
EXTRA_BASES = {"tuple"} if module == "typing" else set()


def base_names(cls):
    out = []
    for b in cls.bases:
        if isinstance(b, ast.Subscript):
            b = b.value
        if isinstance(b, ast.Name) and (b.id in top_names or b.id in EXTRA_BASES) and b.id != cls.name:
            out.append(b.id)
    return out


def decorators(fn):
    return [d.id for d in fn.decorator_list if isinstance(d, ast.Name) and d.id in KEEP_DECORATORS]


def emit_function(fn, indent, in_class, lines):
    decs = decorators(fn)
    for d in decs:
        lines.append(f"{indent}@{d}")
    if in_class and "staticmethod" not in decs:
        implicit_cls = fn.name in ("__new__", "__init_subclass__", "__class_getitem__")
        first = "cls" if ("classmethod" in decs or implicit_cls) else "self"
        params = f"{first}, *args, **kwargs"
    else:
        params = "*args, **kwargs"
    lines.append(f"{indent}def {fn.name}({params}): ...")


def alias_target(name, in_class):
    """The class that a typing alias stands for, or "None"."""
    if module == "typing" and not in_class:
        return TYPING_ALIASES.get(name, "None")
    return "None"


def emit_body(body, indent, in_class, lines, skip_names=()):
    seen = set(skip_names)
    count = 0
    for stmt in flatten(body):
        if isinstance(stmt, ast.FunctionDef):
            if stmt.name in seen or (in_class and not public(stmt.name)):
                continue
            if not in_class and not public(stmt.name):
                continue
            # Skip property setters and deleters.
            if any(isinstance(d, ast.Attribute) and d.attr in ("setter", "deleter") for d in stmt.decorator_list):
                continue
            seen.add(stmt.name)
            emit_function(stmt, indent, in_class, lines)
            count += 1
        elif isinstance(stmt, ast.ClassDef):
            if stmt.name in seen or not public(stmt.name):
                continue
            seen.add(stmt.name)
            emit_class(stmt, indent, lines)
            count += 1
        elif isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name):
            n = stmt.target.id
            if n in seen or not public(n):
                continue
            seen.add(n)
            lines.append(f"{indent}{n} = {alias_target(n, in_class)}")
            count += 1
        elif isinstance(stmt, ast.Assign):
            for t in stmt.targets:
                if not isinstance(t, ast.Name) or t.id in seen or not public(t.id):
                    continue
                seen.add(t.id)
                if alias_target(t.id, in_class) != "None":
                    lines.append(f"{indent}{t.id} = {alias_target(t.id, in_class)}")
                    count += 1
                    continue
                # Keep aliases of module names (EnvironmentError = OSError). Others get None.
                if not in_class and isinstance(stmt.value, ast.Name) and stmt.value.id in top_names:
                    lines.append(f"{indent}{t.id} = {stmt.value.id}")
                else:
                    lines.append(f"{indent}{t.id} = None")
                count += 1
    return count


def emit_class(cls, indent, lines):
    bases = base_names(cls)
    head = f"{indent}class {cls.name}"
    if bases:
        head += "(" + ", ".join(bases) + ")"
    lines.append(head + ":")
    n = emit_body(cls.body, indent + "    ", True, lines)
    if n == 0:
        lines.append(f"{indent}    pass")
    lines.append("")


DESCRIPTIONS = {
    "builtins": "the builtin names",
    "typing": "the names of the typing module",
}
header = f"""# Python {module} stub for the stack graphs rules.
#
# Derived from typeshed, stdlib/{module}.pyi, commit {commit}.
# Source: https://github.com/python/typeshed
# Copyright (c) 2015 Jukka Lehtosalo and contributors.
# typeshed is licensed under the Apache License, Version 2.0, with parts under the MIT license.
# See https://github.com/python/typeshed/blob/main/LICENSE
#
# This file keeps {DESCRIPTIONS[module]} and the class structure only.
# It drops all typing syntax, signatures, and private names.
"""
if module == "typing":
    header += """# Typeshed declares List, Dict, Set, FrozenSet, Tuple, and Type with no target.
# It declares Text as an alias of str.
# Here each one is an alias of the builtin class that it stands for.
"""
header += "# Generated by script/gen_stubs.py. Do not edit by hand.\n"


def all_names():
    """The names that the typing module lists in `__all__`, with both sides of version checks."""
    names = []
    for stmt in flatten(tree.body):
        value = None
        if isinstance(stmt, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "__all__" for t in stmt.targets):
            value = stmt.value
        elif isinstance(stmt, ast.AugAssign) and isinstance(stmt.target, ast.Name) and stmt.target.id == "__all__":
            value = stmt.value
        if isinstance(value, (ast.List, ast.Tuple)):
            names += [e.value for e in value.elts if isinstance(e, ast.Constant) and isinstance(e.value, str)]
    return names


lines = []
if module == "builtins":
    for name in MODULE_CONSTANTS:
        lines.append(f"{name} = None")
    lines.append("")
    emit_body(tree.body, "", False, lines, skip_names=MODULE_CONSTANTS)
else:
    emit_body(tree.body, "", False, lines, skip_names=["__all__"])
    # A name that the module re-exports from another module has no definition here.
    # Define it as an opaque name, so that `from typing import Match` resolves.
    defined = {m.group(1) or m.group(2) for l in lines for m in [re.match(r"(?:class|def) (\w+)|(\w+) = ", l)] if m}
    extra = [n for n in all_names() if n not in defined and public(n)]
    for n in sorted(set(extra)):
        lines.append(f"{n} = None")
text = header + "\n" + "\n".join(lines).rstrip() + "\n"
open(out_path, "w", encoding="utf-8").write(text)
# Check that Python itself parses the stub.
ast.parse(text)
print(f"wrote {out_path}: {len(text.splitlines())} lines, {len(text)} bytes")

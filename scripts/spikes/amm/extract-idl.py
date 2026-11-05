#!/usr/bin/env python3
"""Extract the cp_amm IDL embedded in the @meteora-ag/cp-amm-sdk npm bundle, without executing any of it.

    npm pack @meteora-ag/cp-amm-sdk@1.5.1 && mkdir pkg && tar xzf meteora-ag-cp-amm-sdk-1.5.1.tgz -C pkg
    python3 -I scripts/spikes/amm/extract-idl.py pkg/package/dist/index.js scripts/spikes/amm/idls/cp_amm.json

The bundle holds the IDL as a JavaScript object literal. This script brace-matches that literal, lexes it
strictly (strings, numbers, true/false/null, bare keys; anything else aborts), converts it to JSON and writes
it. Whole-valued floats such as `6e3` become integers, because Anchor's IDL parser wants integers.
"""
import json
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
s = open(src, encoding="utf-8").read()
marker = "var cp_amm_default = "
start = s.index(marker) + len(marker)
assert s[start] == "{", "IDL literal not found"

i, depth, in_str = start, 0, False
while True:
    c = s[i]
    if in_str:
        if c == "\\":
            i += 1
        elif c == '"':
            in_str = False
    elif c == '"':
        in_str = True
    elif c == "{":
        depth += 1
    elif c == "}":
        depth -= 1
        if depth == 0:
            break
    i += 1
lit = s[start : i + 1]

num = re.compile(r"-?\d+(\.\d+)?([eE][+-]?\d+)?")
out, j, in_str = [], 0, False
while j < len(lit):
    c = lit[j]
    if in_str:
        out.append(c)
        if c == "\\":
            j += 1
            out.append(lit[j])
        elif c == '"':
            in_str = False
        j += 1
        continue
    if c == '"':
        in_str = True
        out.append(c)
        j += 1
        continue
    if c.isdigit() or c == "-":
        m = num.match(lit, j)
        if not m:
            sys.exit(f"bad number at {j}")
        out.append(m.group(0))
        j = m.end()
        continue
    if c.isalpha() or c == "_":
        k = j
        while k < len(lit) and (lit[k].isalnum() or lit[k] == "_"):
            k += 1
        word, rest = lit[j:k], lit[k:].lstrip()
        if rest.startswith(":"):
            out.append('"' + word + '"')
        elif word in ("true", "false", "null"):
            out.append(word)
        else:
            sys.exit(f"unexpected token outside strings: {word!r}")
        j = k
        continue
    if c in " \t\r\n{}[],:":
        out.append(c)
        j += 1
        continue
    sys.exit(f"unexpected character outside strings: {c!r}")

idl = json.loads(re.sub(r",(\s*[}\]])", r"\1", "".join(out)))


def ints(x):
    if isinstance(x, float) and x.is_integer():
        return int(x)
    if isinstance(x, list):
        return [ints(v) for v in x]
    if isinstance(x, dict):
        return {k: ints(v) for k, v in x.items()}
    return x


idl = ints(idl)
with open(dst, "w", encoding="utf-8") as f:
    json.dump(idl, f, indent=2)
    f.write("\n")
print("wrote", dst, idl["metadata"], "instructions:", len(idl["instructions"]))

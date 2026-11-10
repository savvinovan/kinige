#!/usr/bin/env python3
"""Self-check of a branch before its PR (KINIG-110): the findings reviewers kept returning that a script can see.

Usage: python3 -I scripts/pre-pr-check.py [--base REF]  (run it in the branch's checkout or worktree)

Checks the files changed against the merge base with `origin/main` (committed, uncommitted and untracked):
- a code file that grew past 400 lines in this branch (CLAUDE.md, rule 6), unless `scripts/pre-pr-check.allow` lists it
  with a reason; migrations and generated files are exempt;
- leftover backup files (`.bak`, `.orig`, `.rej`, `.swp`, `~`);
- invisible characters (zero-width, bidi controls, BOM, soft hyphen) in added lines;
- a Markdown table with an added line whose columns are not aligned (CLAUDE.md, code style 4);
- trailing whitespace and conflict markers in added lines;
- `cargo fmt --all --check` when Rust files changed.

Exit codes: 0 clean, 1 findings (listed on stdout), 2 the check could not run. CI runs it on every pull request (the
`pre-pr-check` job, against the PR's base); implementers and reviewers run it in their worktrees before that.
Tests: `python3 -I scripts/pre-pr-check.test.py`.
"""

import os
import re
import shutil
import subprocess
import sys
import unicodedata

MAX_LINES = 400
CODE_EXT = (".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".py", ".sh", ".ps1", ".css")
EXEMPT = ("crates/kinige-store/migrations/", "server/migrations/", "web/src/api/generated/", "web/src/ui/forest/fixtures/",
          "web/src/ui/tokens.css", "web/src/ui/themes.css")
BACKUP_RE = re.compile(r"(\.bak|\.orig|\.rej|\.swp|~)$")
# Zero-width space and non-joiner, LRM and RLM, bidi embeddings and isolates, word joiner and invisible operators, BOM,
# soft hyphen. The zero-width joiner U+200D is left out: emoji sequences need it. Built from code points so that this
# file itself stays plain ASCII.
INVISIBLE = (0x200B, 0x200C, 0x200E, 0x200F, *range(0x202A, 0x202F), *range(0x2060, 0x2065), *range(0x2066, 0x206A),
             0xFEFF, 0x00AD)
INVISIBLE_RE = re.compile("[" + "".join(map(chr, INVISIBLE)) + "]")
ALLOW_FILE = "scripts/pre-pr-check.allow"


def main(argv):
    base_ref = argv[argv.index("--base") + 1] if "--base" in argv and argv.index("--base") + 1 < len(argv) else "origin/main"
    try:
        findings = run_checks(os.getcwd(), base_ref)
    except CheckError as err:
        print("pre-pr-check: could not run: {}".format(err), file=sys.stderr)
        return 2
    if not findings:
        return 0
    print("pre-pr-check found {} problem(s):\n".format(len(findings)) + "\n".join("- " + f for f in findings))
    print("Fix them with Edit (not scripts); this check is scripts/pre-pr-check.py.")
    return 1


class CheckError(Exception):
    pass


def run_checks(cwd, base_ref):
    root = git(cwd, "rev-parse", "--show-toplevel").strip()
    base = git(root, "merge-base", base_ref, "HEAD").strip()
    changed = [p for p in git(root, "diff", "--name-only", "--diff-filter=AM", base).splitlines() if p]
    untracked = [p for p in git(root, "ls-files", "--others", "--exclude-standard").splitlines() if p]
    files = sorted(set(changed) | set(untracked))
    added = added_lines(root, base, untracked)
    findings = []
    findings += long_files(root, base, files)
    findings += [p + ": a backup file; delete it" for p in files if BACKUP_RE.search(p)]
    findings += invisible(added)
    findings += tables(root, files, added)
    findings += whitespace(added)
    if any(p.endswith(".rs") for p in files):
        findings += rust_fmt(root)
    return findings


def git(cwd, *args):
    try:
        out = subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError) as err:
        raise CheckError("git {}: {}".format(args[0], err)) from err
    if out.returncode != 0:
        raise CheckError("git {}: {}".format(" ".join(args[:2]), out.stderr.strip()[:200]))
    return out.stdout


def added_lines(root, base, untracked):
    """{path: {line number: text}} of the lines this branch adds, untracked files whole."""
    result = {}
    path, line = None, 0
    for raw in git(root, "diff", "-U0", "--no-color", base).splitlines():
        if raw.startswith("+++ "):
            path = raw[6:] if raw.startswith("+++ b/") else None
        elif raw.startswith("@@"):
            m = re.match(r"@@ -\S+ \+(\d+)", raw)
            line = int(m.group(1)) if m else 0
        elif path and raw.startswith("+"):
            result.setdefault(path, {})[line] = raw[1:]
            line += 1
    for p in untracked:
        text = read(os.path.join(root, p))
        if text is not None:
            result[p] = {i + 1: t for i, t in enumerate(text.splitlines())}
    return result


def long_files(root, base, files):
    allowed = allow_list(root)
    out = []
    for p in files:
        if not p.endswith(CODE_EXT) or p.startswith(EXEMPT) or p in allowed:
            continue
        text = read(os.path.join(root, p))
        if text is None:
            continue
        now = len(text.splitlines())
        try:
            before = len(git(root, "show", "{}:{}".format(base, p)).splitlines())
        except CheckError:
            before = 0
        if now > MAX_LINES >= before:
            out.append("{}: {} lines, over {} (split it, or list it in {} with a reason)".format(p, now, MAX_LINES, ALLOW_FILE))
    return out


def allow_list(root):
    text = read(os.path.join(root, ALLOW_FILE)) or ""
    return {line.split("#")[0].strip() for line in text.splitlines() if line.split("#")[0].strip()}


def invisible(added):
    out = []
    for p, lines in sorted(added.items()):
        for n, text in sorted(lines.items()):
            m = INVISIBLE_RE.search(text)
            if m:
                out.append("{}:{}: invisible character U+{:04X}".format(p, n, ord(m.group(0))))
    return out


def tables(root, files, added):
    out = []
    for p in files:
        if not p.endswith(".md") or p not in added:
            continue
        text = read(os.path.join(root, p))
        if text is None:
            continue
        for start, rows in table_blocks(text.splitlines()):
            touched = any(start + i in added[p] for i in range(len(rows)))
            if touched and len({tuple(pipe_columns(r)) for r in rows}) > 1:
                out.append("{}:{}: the table's columns are not aligned with spaces".format(p, start))
    return out


def table_blocks(lines):
    """(first line number, rows) for each run of lines that start with `|` outside code fences."""
    blocks, rows, start, fence = [], [], 0, False
    for i, line in enumerate(lines + [""], start=1):
        if line.lstrip().startswith("```"):
            fence = not fence
        if not fence and line.lstrip().startswith("|"):
            if not rows:
                start = i
            rows.append(line.rstrip())
        else:
            if len(rows) > 1:
                blocks.append((start, rows))
            rows = []
    return blocks


def pipe_columns(row):
    """Display columns of the unescaped `|` separators of a row (wide characters count as two)."""
    cols, col, prev = [], 0, ""
    for c in row:
        if c == "|" and prev != "\\":
            cols.append(col)
        col += 2 if unicodedata.east_asian_width(c) in ("W", "F") else (0 if unicodedata.combining(c) else 1)
        prev = c
    return cols


def whitespace(added):
    """Trailing whitespace and conflict markers in added lines (`git diff --check` would miss untracked files)."""
    out = []
    for p, lines in sorted(added.items()):
        for n, text in sorted(lines.items()):
            if text != text.rstrip(" \t"):
                out.append("{}:{}: trailing whitespace".format(p, n))
            elif re.match(r"(<{7}|>{7}) |={7}$", text):
                out.append("{}:{}: a conflict marker".format(p, n))
    return out[:20]


def rust_fmt(root):
    """A finding when rustfmt would change a file; nothing when cargo is missing or fails for another reason (CI checks it too)."""
    if not shutil.which("cargo") or not os.path.isfile(os.path.join(root, "Cargo.toml")):
        return []
    out = subprocess.run(["cargo", "fmt", "--all", "--check"], cwd=root, capture_output=True, text=True, timeout=300)
    if out.returncode != 0 and "Diff in " in out.stdout:
        return ["cargo fmt --all --check fails: run `cargo fmt --all`"]
    return []


def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except (OSError, UnicodeDecodeError):
        return None


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

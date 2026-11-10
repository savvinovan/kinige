#!/usr/bin/env python3
"""Kinige's tool-call guard (KINIG-110): the CLAUDE.md rules that prose alone did not hold, enforced by the harness.

Modes (`.claude/settings.json` runs them):
- `bash`: PreToolUse for Bash. Blocks string-replacement edits and shell writes into sources (rule 8), the user's real
  data, GUI and system settings on the user's Mac, Claude configs, pushes to main and force pushes, pushes of commits
  in which gitleaks finds a secret, docker commands on other projects' objects, git in the main checkout by a
  subagent, and cargo builds on a nearly full disk. Then it records the snapshot for `post`.
- `post`: PostToolUse for Bash. Reports source files the command rewrote by script (`snapshot.py`).
- `paths`: PreToolUse for the file tools. Blocks the user's real data, writes to Claude configs, and edits by a
  subagent in the main checkout instead of its worktree.

Exit 0 lets the call run; exit 2 blocks it (PreToolUse) or reports to the agent (PostToolUse); stderr says why and what
to do instead. An internal error lets the call through: a broken guard must not stop the work. The tests are
`.claude/hooks/tests/test_hooks.py` (CI, the `web` job).
"""

import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import snapshot  # noqa: E402  (the guard's own module, next to it)

HOME_PREFIX = r"(?:~|\$HOME|\$\{HOME\}|/Users/[^/\s'\"]+)/"
USER_DATA_RE = re.compile(
    HOME_PREFIX + r"(?:Library/Application(?:\\ | |%20)Support/me\.savvinov\.kinige|Library/Logs/me\.savvinov\.kinige"
    r"|Documents/Kinige(?![\w.-]))"
)
USER_DATA_DIRS = ("Library/Application Support/me.savvinov.kinige", "Library/Logs/me.savvinov.kinige", "Documents/Kinige")
CLAUDE_CONFIGS = (".claude.json", ".claude/settings.json", ".claude/settings.local.json")

SOURCE_PATH_RE = re.compile(
    r"(?:^|/)(?:crates|src-tauri|web/src|web/\.storybook|server|spikes|scripts|docs|design|deploy|fixtures|\.github"
    r"|\.claude/(?:agents|hooks|skills))/\S+"
    r"|(?:^|/)(?:Cargo\.toml|CLAUDE\.md|README\.md|deny\.toml|ATTRIBUTION\.md|rust-toolchain\.toml|clippy\.toml"
    r"|\.gitleaksignore|package\.json|vite\.config\.ts)$"
)
INLINE_WRITE_RE = re.compile(
    r"write_text\(|write_bytes\(|\.write\(|open\([^)]*['\"][wax]b?\+?['\"]|writeFileSync|writeFile\(|fs\.write"
    r"|os\.replace\(|shutil\.(?:copy|move)"
)
INLINE_PATH_RE = re.compile(
    r"(?:crates|src-tauri|web/src|server|spikes|scripts|docs|design|deploy|fixtures|\.github)/[\w./-]+"
    r"|Cargo\.toml|CLAUDE\.md|README\.md|deny\.toml"
)
SHELLS = ("bash", "sh", "zsh")
GUI_SCRIPTS = ("run-app.sh", "screenshot-window.sh", "measure-cold-start.sh")
GUI_PROGRAMS = {"osascript", "screencapture", "cliclick", "tccutil"}
CARGO_HEAVY = {"build", "test", "clippy", "check", "run", "doc", "bench", "nextest", "tauri", "install"}
# The tests raise the floor through the environment of the hook process (an agent's command cannot set it).
MIN_FREE_GIB = float(os.environ.get("KINIGE_GUARD_MIN_FREE_GIB") or 6)
GIT_MUTATING = {"checkout", "switch", "commit", "merge", "rebase", "reset", "stash", "pull", "cherry-pick", "revert", "am"}
DOCKER_REMOVING = {"rm", "stop", "kill", "restart", "pause", "rmi", "down"}
WORKTREE_DIR = os.path.join(".claude", "worktrees")
MEMORY_DIRS = (os.path.join(".claude", "agent-memory"), os.path.join(".claude", "agent-memory-local"))

RULE8 = (
    "CLAUDE.md rule 8: no edits to source files with string-replacement scripts or shell writes. Use the Edit tool for a "
    "change and the Write tool for a new file. A reviewer's fault-injection probe goes through `git apply` of a patch "
    "file in its own worktree (and `git checkout -- .` afterwards)."
)


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    try:
        data = json.load(sys.stdin)
    except ValueError:
        return 0
    if not isinstance(data, dict):
        return 0
    checks = {"bash": check_bash, "post": check_post, "paths": check_paths}
    try:
        problem = checks[mode](data) if mode in checks else None
    except Exception as err:  # noqa: BLE001 (fail open, see the module doc)
        print("kinige guard: internal error ({}), the call was not checked".format(type(err).__name__), file=sys.stderr)
        return 0
    if problem:
        lead = "Reported by" if mode == "post" else "Blocked by"
        print("{} .claude/hooks/guard.py: {}".format(lead, problem), file=sys.stderr)
        return 2
    return 0


def check_bash(data):
    command = str((data.get("tool_input") or {}).get("command") or "")
    cwd = str(data.get("cwd") or os.getcwd())
    problem = check_command(command, data, cwd, depth=0)
    if problem:
        return problem
    snapshot.save(data, cwd)
    return None


def check_command(command, data, cwd, depth):
    skeleton, bodies = shell_skeleton(command)
    workdir = cwd
    for words in split_segments(skeleton):
        words = unwrap(words)
        if not words:
            continue
        prog, args = words[0], words[1:]
        name = re.sub(r"^(python)3?[\d.]*$", r"\g<1>3", os.path.basename(prog))
        # A commit message or PR body may name the data paths (git, gh); an inline script's text is all of the command.
        seen = command if is_inline_script(name, args, words) else " ".join(bodies.get(w, w) for w in words)
        if name not in ("git", "gh") and USER_DATA_RE.search(seen):
            return ("this command touches the user's real Kinige data (Application Support, Logs or ~/Documents/Kinige). "
                    "Never open or change it; use a temporary KINIGE_DATA_DIR and KINIGE_MIRROR_DIR with synthetic data.")
        if name == "cd" and args:
            workdir = os.path.normpath(os.path.join(workdir, os.path.expanduser(args[0])))
            continue
        if name in SHELLS and "-c" in args and depth < 3:
            inner = args[args.index("-c") + 1] if args.index("-c") + 1 < len(args) else ""
            problem = check_command(bodies.get(inner, inner), data, workdir, depth + 1)
            if problem:
                return problem
            continue
        problem = (check_edit(name, args, words, command) or check_gui(prog, name, args)
                   or check_claude(name, args, words) or check_git(name, args, data, workdir)
                   or check_docker(name, args, command) or check_disk(name, args, workdir))
        if problem:
            return problem
    return None


def check_post(data):
    command = str((data.get("tool_input") or {}).get("command") or "")
    skeleton, _ = shell_skeleton(command)
    programs = [(w[0], w[1] if len(w) > 1 else "") for w in (unwrap(s) for s in split_segments(skeleton)) if w]
    changed = snapshot.compare(data, programs)
    if not changed:
        return None
    shown = ", ".join(changed[:10]) + (" and {} more".format(len(changed) - 10) if len(changed) > 10 else "")
    return ("this Bash command rewrote source files: {}. {} Revert these files (`git checkout -- <path>` for tracked ones, "
            "delete new ones) and redo the change with Edit or Write. If the command is a generator that is meant to write "
            "sources, say so in your report so the coordinator adds it to .claude/hooks/snapshot.py.").format(shown, RULE8)


def check_paths(data):
    tool = str(data.get("tool_name") or "")
    tin = data.get("tool_input") or {}
    raw = tin.get("file_path") or tin.get("notebook_path") or tin.get("path") or ""
    if not raw:
        return None
    path = os.path.realpath(os.path.expanduser(str(raw)))
    home = os.path.realpath(os.path.expanduser("~"))
    for d in USER_DATA_DIRS:
        base = os.path.join(home, d)
        if path == base or path.startswith(base + os.sep):
            return "this path is the user's real Kinige data. Never open or change it; use synthetic data in a temp dir."
    writing = tool in ("Edit", "Write", "NotebookEdit", "MultiEdit")
    if writing and any(path == os.path.join(home, c) for c in CLAUDE_CONFIGS):
        return "Claude configs are the user's: do not change them from this project (CLAUDE.md, Environment)."
    project = os.path.realpath(os.environ.get("CLAUDE_PROJECT_DIR") or "")
    if writing and data.get("agent_type") and project and in_main_checkout(path, project):
        return ("a subagent edits the main checkout ({}). Work in your own worktree (CLAUDE.md rule 13); the main checkout "
                "belongs to the coordinator.").format(project)
    return None


def check_edit(name, args, words, command):
    if name in ("sed", "gsed") and any(re.match(r"-[a-zA-Z]*i", a) or a.startswith("--in-place") for a in args):
        return "`sed -i` edits files in place. " + RULE8
    if name == "perl" and any(re.match(r"-(?![MI])[a-zA-Z0-9]*i", a) for a in args):
        return "`perl -i` edits files in place. " + RULE8
    target = redirect_target(words) or tee_target(name, args)
    if target and SOURCE_PATH_RE.search(target):
        return "the command writes into {} through the shell. {}".format(target, RULE8)
    if is_inline_script(name, args, words) and INLINE_WRITE_RE.search(command) and INLINE_PATH_RE.search(command):
        return "an inline {} script writes a repository file. {}".format(name, RULE8)
    return None


def is_inline_script(name, args, words):
    """python/node/... running code from the command line or a heredoc (not a script file)."""
    return name in snapshot.INTERPRETERS and name not in SHELLS and (
        any(a in ("-c", "-e", "-", "--eval", "-p") for a in args) or "<<" in words)


def check_gui(prog, name, args):
    if name in GUI_PROGRAMS:
        return ("`{}` drives the user's Mac (GUI, input, screenshots or privacy settings). GUI checks run only on GitHub "
                "runners through screens.yml.").format(name)
    if name == "open" and any(a.rstrip("/").endswith(".app") or a == "-a" or a.startswith("x-apple") for a in args):
        return "`open` launches an app on the user's Mac. Acceptance builds are opened by the user; GUI checks run on screens.yml."
    if name == "defaults" and args and args[0] in ("write", "delete"):
        return "`defaults write` changes macOS settings on the user's Mac. Never change system settings."
    script = prog if name.endswith(".sh") else (args[0] if name in SHELLS and args else "")
    if os.path.basename(script) in GUI_SCRIPTS:
        return "`{}` opens the Kinige window on the user's Mac. Use screens.yml on a GitHub runner.".format(os.path.basename(script))
    return None


def check_claude(name, args, words):
    if name == "claude" and args and args[0] in ("mcp", "config"):
        return "do not register servers or change Claude configs from this project (kinige-mcp is never registered)."
    target = redirect_target(words) or tee_target(name, args)
    if target and any(target.endswith(c) for c in CLAUDE_CONFIGS):
        return "Claude configs are the user's: do not write them from this project."
    return None


def check_git(name, args, data, workdir):
    if name != "git":
        return None
    sub, rest, repo = git_subcommand(args, workdir)
    if sub in GIT_MUTATING and data.get("agent_type"):
        project = os.path.realpath(os.environ.get("CLAUDE_PROJECT_DIR") or "")
        if project and in_main_checkout(os.path.realpath(repo), project):
            return "a subagent runs `git {}` in the main checkout. Work in your own worktree (CLAUDE.md rule 13).".format(sub)
    if sub != "push":
        return None
    for a in rest:
        if a in ("--force", "--all", "--mirror") or (re.match(r"-[a-zA-Z]*f[a-zA-Z]*$", a) and not a.startswith("--")):
            return "force pushes and `--all`/`--mirror` are not allowed. Use `--force-with-lease` on your own branch."
    refspecs = [a for a in rest if not a.startswith("-")][1:]
    for ref in refspecs:
        if ref.split(":")[-1].lstrip("+") in ("main", "master", "refs/heads/main", "refs/heads/master"):
            return "pushing to main is not allowed: changes reach main only through a reviewed PR."
    return gitleaks_problem(repo, refspecs)


def check_docker(name, args, command):
    if name != "docker":
        return None
    if "prune" in args:
        return "`docker … prune` would remove other projects' objects on ubuntu-server. Remove your own `kinige-` objects by name."
    # The whole command line, so `docker rm -f $(docker ps -aq --filter name=kinige-)` counts as targeting ours.
    if DOCKER_REMOVING & set(args) and "kinige" not in command:
        return "this docker command targets containers, images or volumes not named `kinige-…`. Touch only your own objects."
    return None


def check_disk(name, args, workdir):
    if name != "cargo" or not CARGO_HEAVY & {a for a in args if not a.startswith("-")}:
        return None
    probe = workdir if os.path.isdir(workdir) else os.getcwd()
    try:
        free = shutil.disk_usage(probe).free / 1024 ** 3
    except OSError:
        return None
    if free < MIN_FREE_GIB:
        return ("only {:.1f} GiB free (the floor is {} GiB). Delete your own CARGO_TARGET_DIR or other files you created, "
                "or report to the coordinator; do not clean shared caches.").format(free, MIN_FREE_GIB)
    return None


def gitleaks_problem(repo, refspecs):
    if not shutil.which("gitleaks"):
        return None
    src = refspecs[0].split(":")[0].lstrip("+") if refspecs else ""
    try:
        out = subprocess.run(
            ["gitleaks", "git", "--no-banner", "--redact", "--exit-code", "3",
             "--log-opts", "{} --not --remotes".format(src or "HEAD"), repo],
            capture_output=True, text=True, timeout=90,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if out.returncode == 3:
        tail = "\n".join((out.stdout + out.stderr).strip().splitlines()[-25:])
        return ("gitleaks found a secret in the commits to push (the repository is public). Fix the commit, or add a "
                "true false positive to .gitleaksignore.\n" + tail)
    return None


def git_subcommand(args, workdir):
    repo, i = workdir, 0
    while i < len(args) and args[i].startswith("-"):
        if args[i] == "-C" and i + 1 < len(args):
            repo = os.path.normpath(os.path.join(repo, os.path.expanduser(args[i + 1])))
            i += 2
        elif args[i] in ("-c", "--git-dir", "--work-tree") and i + 1 < len(args):
            i += 2
        else:
            i += 1
    if i >= len(args):
        return "", [], repo
    return args[i], args[i + 1:], repo


def in_main_checkout(path, project):
    if not (path == project or path.startswith(project + os.sep)):
        return False
    rel = os.path.relpath(path, project)
    return not (rel.startswith(WORKTREE_DIR) or rel.startswith(MEMORY_DIRS))


def unwrap(words):
    """The command that `xargs`, `find -exec` or `nice` run, so `xargs sed -i` is checked as `sed -i`."""
    while words:
        name = os.path.basename(words[0])
        if name in ("xargs", "nice", "timeout", "caffeinate"):
            rest, i = words[1:], 0
            while i < len(rest) and (rest[i].startswith("-") or (name == "timeout" and i == 0)):
                i += 2 if rest[i] in ("-n", "-I", "-L", "-P", "-d", "-s", "-E") else 1
            words = rest[i:]
        elif name == "find" and ("-exec" in words or "-execdir" in words):
            key = "-exec" if "-exec" in words else "-execdir"
            words = [w for w in words[words.index(key) + 1:] if w not in (";", "\\;", "+", "{}")]
        else:
            return words
    return words


def redirect_target(words):
    for i, w in enumerate(words):
        m = re.match(r"^\d?(?:>>?|&>>?)(.*)$", w)
        if not m:
            continue
        target = m.group(1) or (words[i + 1] if i + 1 < len(words) else "")
        if target and not target.startswith("&") and target != "/dev/null":
            return target
    return ""


def tee_target(name, args):
    if name != "tee":
        return ""
    files = [a for a in args if not a.startswith("-")]
    return files[0] if files else ""


def shell_skeleton(command):
    """The command without heredoc bodies, and its quoted strings.

    A quoted string becomes its text when it is one word (a path, a sed script), otherwise a `QSTRn` placeholder whose
    text is in the returned dict (for `bash -c`). Commit messages and PR bodies are quoted or heredocs, so an arrow or a
    path inside them is never read as a redirect.
    """
    text = re.sub(r"<<-?\s*(['\"]?)(\w+)\1([^\n]*)\n.*?\n\s*\2[ \t]*(?=\n|$)", r"<< \3", command, flags=re.S)
    out, bodies, i = [], {}, 0
    while i < len(text):
        c = text[i]
        if c in "'\"":
            j = text.find(c, i + 1)
            while c == '"' and j > 0 and text[j - 1] == "\\":
                j = text.find(c, j + 1)
            if j < 0:
                j = len(text)
            body = text[i + 1:j]
            if body and not re.search(r"\s", body):
                out.append(body)
            else:
                key = "QSTR{}".format(len(bodies))
                bodies[key] = body
                out.append(key)
            i = j + 1
            continue
        out.append(c)
        i += 1
    return "".join(out), bodies


def split_segments(skeleton):
    """Words of each simple command, split on `&&`, `||`, `;`, `|`, `(`, `)`, `$(` and newlines; leading VAR=x dropped."""
    segments = []
    for part in re.split(r"&&|\|\||[;|\n()`]|\$\(", skeleton):
        words = re.findall(r"\d?&?>>?\S*|<<|\S+", part)
        while words and (re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0])
                         or words[0] in ("sudo", "time", "command", "exec", "env", "nohup")):
            words = words[1:]
        segments.append(words)
    return segments


if __name__ == "__main__":
    sys.exit(main())

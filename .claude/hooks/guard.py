#!/usr/bin/env python3
"""Kinige's tool-call guard (KINIG-110): the CLAUDE.md rules that prose alone did not hold, enforced by the harness.

Modes (`.claude/settings.json` runs them):
- `bash`: PreToolUse for Bash. Blocks string-replacement edits and shell writes into sources (rule 8), the user's real
  data, GUI and system settings on the user's Mac, writes to Claude configs, pushes to main and force pushes, pushes of
  commits in which gitleaks finds a secret, docker commands on other projects' objects, git that changes the main
  checkout when a subagent runs it, and cargo builds on a nearly full disk. Then it records the snapshot for `post`.
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

sys.dont_write_bytecode = True  # no __pycache__ in the checkout the hook runs from
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import shellparse  # noqa: E402  (the guard's own modules, next to it)
import snapshot  # noqa: E402

HOME_PREFIX = r"(?:~|\$HOME|\$\{HOME\}|/Users/[^/\s'\"]+)/"
USER_DATA_RE = re.compile(
    HOME_PREFIX + r"(?:Library/Application(?:\\ | |%20)Support/me\.savvinov\.kinige|Library/Logs/me\.savvinov\.kinige"
    r"|Documents/Kinige(?![\w.-]))"
)
USER_DATA_DIRS = ("Library/Application Support/me.savvinov.kinige", "Library/Logs/me.savvinov.kinige", "Documents/Kinige")
CLAUDE_CONFIGS = (".claude.json", ".claude/settings.json", ".claude/settings.local.json")
# The user's own configs (under the home directory), not a project's `.claude/settings.json`.
CLAUDE_CONFIG_RE = re.compile(HOME_PREFIX + r"\.claude(?:\.json|/settings(?:\.local)?\.json)")
INLINE_WRITE_RE = re.compile(
    r"write_text\(|write_bytes\(|\.write\(|open\([^)]*['\"][wax]b?\+?['\"]|writeFileSync|writeFile\(|fs\.write"
    r"|os\.replace\(|shutil\.(?:copy|move)"
)
INLINE_PATH_RE = re.compile(
    r"(?:crates|src-tauri|web/src|server|spikes|scripts|docs|design|deploy|fixtures|\.github)/[\w./-]+"
    r"|Cargo\.toml|CLAUDE\.md|README\.md|deny\.toml"
)
SHELLS = ("bash", "sh", "zsh")
FILE_COPIERS = {"cp", "mv", "install", "rsync", "ln", "dd"}
GUI_SCRIPTS = ("run-app.sh", "screenshot-window.sh", "measure-cold-start.sh")
GUI_PROGRAMS = {"open", "osascript", "screencapture", "cliclick", "tccutil", "killall"}
CARGO_HEAVY = {"build", "test", "clippy", "check", "run", "doc", "bench", "nextest", "tauri", "install"}
# The tests raise the floor through the environment of the hook process (an agent's command cannot set it).
MIN_FREE_GIB = float(os.environ.get("KINIGE_GUARD_MIN_FREE_GIB") or 6)
# What a subagent may run in the main checkout: git that does not change it. Everything else is blocked there.
GIT_READ_ONLY = {
    "status", "log", "diff", "show", "fetch", "rev-parse", "ls-files", "ls-tree", "blame", "grep", "merge-base",
    "cat-file", "describe", "shortlog", "ls-remote", "for-each-ref", "rev-list", "name-rev", "check-ignore", "version",
    "help", "push",
}
PUSH_OPTIONS_WITH_VALUE = {"-o", "--push-option", "--repo", "--receive-pack", "--exec"}
DOCKER_REMOVING = {"rm", "stop", "kill", "restart", "pause", "rmi", "down"}
DOCKER_LISTING = {"ps", "ls", "images", "list"}
WORKTREE_DIR = os.path.join(".claude", "worktrees")
MEMORY_DIRS = (os.path.join(".claude", "agent-memory"), os.path.join(".claude", "agent-memory-local"))
SPEC_DIR = os.path.join("docs", "slices") + os.sep

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
    problem, workdir = check_command(command, data, cwd, depth=0)
    if problem:
        return problem
    snapshot.save(data, workdir)
    return None


def check_command(command, data, cwd, depth):
    """(the first problem or None, the directory the line ends up working in)."""
    skel, bodies = shellparse.skeleton(command)
    workdir, env = cwd, {}
    segs = shellparse.segments(skel)
    listing_ours = any(docker_lists_ours(shellparse.command_words(s), bodies) for s in segs)
    for raw in segs:
        remember_assignments(raw, bodies, env)
        raw = [expand(w, env) for w in raw]
        words = shellparse.command_words(raw)
        if not words:
            continue
        prog, args = words[0], words[1:]
        name = re.sub(r"^(python)3?[\d.]*$", r"\g<1>3", os.path.basename(prog))
        # A commit message or PR body may name the data paths (git, gh); an inline script's text is all of the command.
        seen = command if is_inline_script(name, args, words) else " ".join(bodies.get(w, w) for w in raw)
        if name not in ("git", "gh") and USER_DATA_RE.search(seen):
            return ("this command touches the user's real Kinige data (Application Support, Logs or ~/Documents/Kinige). "
                    "Never open or change it; use a temporary KINIGE_DATA_DIR and KINIGE_MIRROR_DIR with synthetic data."), workdir
        if name == "cd" and args:
            workdir = os.path.normpath(os.path.join(workdir, os.path.expanduser(args[0])))
            continue
        if name in SHELLS and "-c" in args and depth < 3:
            at = args.index("-c") + 1
            inner = args[at] if at < len(args) else ""
            problem, _ = check_command(bodies.get(inner, inner), data, workdir, depth + 1)
            if problem:
                return problem, workdir
            continue
        problem = (check_edit(name, args, words, command, workdir) or check_gui(prog, name, args)
                   or check_claude(name, args, words, bodies, command) or check_git(name, args, data, workdir)
                   or check_docker(name, args, listing_ours) or check_disk(name, args, workdir))
        if problem:
            return problem, workdir
    return None, workdir


def remember_assignments(raw, bodies, env):
    """Records `NAME=value` segments (and `export NAME=value`), so a later `git -C "$NAME"` or `cd "$NAME"` in the
    same line is judged by the directory it names. Assignments that prefix a command change only its environment."""
    words = raw[1:] if raw[:1] == ["export"] else raw
    if not words or not all(re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", w) for w in words):
        return
    for w in words:
        key, value = w.split("=", 1)
        env[key] = expand(bodies.get(value, value), env)


def expand(word, env):
    """`word` with `$NAME` and `${NAME}` replaced for the variables the line itself assigned; others stay."""
    if "$" not in word or not env:
        return word
    return re.sub(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?", lambda m: env.get(m.group(1), m.group(0)), word)


def check_post(data):
    command = str((data.get("tool_input") or {}).get("command") or "")
    skel, _ = shellparse.skeleton(command)
    words = [w for w in (shellparse.command_words(s) for s in shellparse.segments(skel)) if w]
    changed = snapshot.compare(data, words)
    if not changed:
        return None
    shown = ", ".join(changed[:10]) + (" and {} more".format(len(changed) - 10) if len(changed) > 10 else "")
    return ("after this Bash command these source files differ from before it: {}. {} If this command made the change, "
            "undo only what it changed (see `git diff`; do not throw away your earlier work in the same files) and make "
            "the change with Edit or Write. If someone else changed the files meanwhile, or the command is a generator "
            "meant to write sources, say so in your report.").format(shown, RULE8)


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
    agent = str(data.get("agent_type") or "")
    if writing and agent and project and in_main_checkout(path, project):
        if agent == "slice-spec" and os.path.relpath(path, project).startswith(SPEC_DIR):
            return None
        return ("a subagent edits the main checkout ({}). Work in your own worktree (CLAUDE.md rule 13); the main checkout "
                "belongs to the coordinator.").format(project)
    return None


def check_edit(name, args, words, command, workdir):
    if name in ("sed", "gsed") and any(re.match(r"-[a-zA-Z]*i", a) or a.startswith("--in-place") for a in args):
        return "`sed -i` edits files in place. " + RULE8
    if name == "perl" and any(re.match(r"-(?![MI])[a-zA-Z0-9]*i", a) for a in args):
        return "`perl -i` edits files in place. " + RULE8
    for target in shellparse.redirect_targets(words) + shellparse.tee_targets(name, args):
        if is_repo_source(target, workdir):
            return "the command writes into {} through the shell. {}".format(target, RULE8)
    if is_inline_script(name, args, words) and INLINE_WRITE_RE.search(command) and INLINE_PATH_RE.search(command):
        return ("an inline {} script writes and names a repository file. {} Analysis that only reads the repository and "
                "writes elsewhere goes in a script file in your scratch directory.").format(name, RULE8)
    return None


def is_inline_script(name, args, words):
    """python/node/... running code from the command line or a heredoc (not a script file)."""
    return name in snapshot.INTERPRETERS and name not in SHELLS and (
        any(a in ("-c", "-e", "-", "--eval", "-p") for a in args) or "<<" in words)


def is_repo_source(target, workdir):
    """True when `target` (relative to `workdir`) is a source file of a Kinige checkout or worktree."""
    if "$" in target:
        return False
    path = os.path.normpath(os.path.join(workdir, os.path.expanduser(target)))
    parent = os.path.dirname(path)
    while parent and not os.path.isdir(parent):
        parent = os.path.dirname(parent)
    root = snapshot.kinige_root(parent)
    if root is None:
        return False
    rel = os.path.relpath(os.path.join(os.path.realpath(parent), os.path.relpath(path, parent)), os.path.realpath(root))
    if not snapshot.is_source(rel):
        return False
    # An ignored file (a coverage log, a build output) is not a source.
    ignored = subprocess.run(["git", "-C", root, "check-ignore", "-q", "--no-index", rel], capture_output=True, timeout=10)
    return ignored.returncode != 0


def check_gui(prog, name, args):
    if name in GUI_PROGRAMS:
        return ("`{}` drives the user's Mac (opening apps or files, input, screenshots, privacy settings, system "
                "processes). GUI checks run only on GitHub runners through screens.yml.").format(name)
    if name == "defaults" and args and args[0] in ("write", "delete"):
        return "`defaults write` changes macOS settings on the user's Mac. Never change system settings."
    script = prog if name.endswith(".sh") else (args[0] if name in SHELLS and args else "")
    if os.path.basename(script) in GUI_SCRIPTS:
        return "`{}` opens the Kinige window on the user's Mac. Use screens.yml on a GitHub runner.".format(os.path.basename(script))
    return None


def check_claude(name, args, words, bodies, command):
    if name == "claude" and args and args[0] in ("mcp", "config"):
        return "do not register servers or change Claude configs from this project (kinige-mcp is never registered)."
    targets = shellparse.redirect_targets(words) + shellparse.tee_targets(name, args)
    files = [bodies.get(a, a) for a in args if not a.startswith("-")]
    if name in FILE_COPIERS and files:
        targets += copy_destinations(name, args, files)
    if any(is_claude_config(t) for t in targets):
        return "Claude configs are the user's: do not write them from this project."
    if is_inline_script(name, args, words) and INLINE_WRITE_RE.search(command) and CLAUDE_CONFIG_RE.search(command):
        return "an inline script writes a Claude config. Claude configs are the user's: do not write them from this project."
    return None


def copy_destinations(name, args, files):
    """Where a copier writes: `of=` for dd, the `-t`/`--target-directory` directory, otherwise the last argument.
    Copying a config out (`cp ~/.claude/settings.json <scratch>`) reads it and is fine."""
    if name == "dd":
        return [a[3:] for a in args if a.startswith("of=")]
    for i, a in enumerate(args):
        directory = (args[i + 1] if a == "-t" and i + 1 < len(args)
                     else a.split("=", 1)[1] if a.startswith("--target-directory=") else None)
        if directory is not None:
            return [os.path.join(directory, os.path.basename(f)) for f in files if f != directory]
    return [files[-1]]


def is_claude_config(target):
    path = os.path.realpath(os.path.expanduser(os.path.expandvars(target)))
    home = os.path.realpath(os.path.expanduser("~"))
    return any(path == os.path.join(home, c) for c in CLAUDE_CONFIGS)


def check_git(name, args, data, workdir):
    if name != "git":
        return None
    sub, rest, repo = shellparse.git_subcommand(args, workdir)
    # A directory named by a variable (`cd "$WT"`) is unknown here, so it is not judged as the main checkout.
    if data.get("agent_type") and "$" not in repo and not git_read_only(sub, rest):
        project = os.path.realpath(os.environ.get("CLAUDE_PROJECT_DIR") or "")
        if project and in_main_checkout(os.path.realpath(repo), project):
            return ("a subagent runs `git {}` in the main checkout ({}); there only git that changes nothing is allowed. "
                    "Work in your own worktree: `cd <worktree> && git …` or `git -C <worktree> …` "
                    "(CLAUDE.md rule 13).").format(sub, project)
    if sub != "push":
        return None
    positional, i = [], 0
    while i < len(rest):
        a = rest[i]
        if a in PUSH_OPTIONS_WITH_VALUE:
            i += 2
            continue
        if a in ("--force", "--all", "--mirror") or (re.match(r"-[a-zA-Z]*f[a-zA-Z]*$", a) and not a.startswith("--")):
            return "force pushes and `--all`/`--mirror` are not allowed. Use `--force-with-lease` on your own branch."
        if not a.startswith("-"):
            positional.append(a)
        i += 1
    refspecs = positional[1:]
    if any(r.startswith("+") for r in refspecs):
        return "a `+refspec` is a force push, which is not allowed. Use `--force-with-lease` on your own branch."
    sources = [("HEAD" if r.split(":")[0] == "@" else r.split(":")[0]) for r in refspecs] or ["HEAD"]
    targets = [r.split(":")[-1] for r in refspecs]
    if "HEAD" in sources and not any(":" in r for r in refspecs):
        targets.append(current_branch(repo))
    if any(t in ("main", "master", "refs/heads/main", "refs/heads/master") for t in targets):
        return "pushing to main is not allowed: changes reach main only through a reviewed PR."
    return gitleaks_problem(repo, sources[0])


def git_read_only(sub, rest):
    if sub in GIT_READ_ONLY:
        return True
    if sub == "worktree":
        # `remove` without --force: git refuses a dirty worktree, so another agent's work is safe.
        return bool(rest) and (rest[0] in ("add", "list", "prune")
                               or (rest[0] == "remove" and not {"-f", "--force"} & set(rest)))
    if sub == "branch":
        return all(a in ("-a", "-r", "-v", "-vv", "--list", "--show-current", "--all", "--remotes") for a in rest)
    if sub == "stash":
        return bool(rest) and rest[0] in ("list", "show")
    if sub in ("remote", "config"):
        return not rest or rest[0] in ("-v", "show", "get-url", "--get", "--list", "-l", "get")
    return False


def current_branch(repo):
    try:
        out = subprocess.run(["git", "-C", repo, "rev-parse", "--abbrev-ref", "HEAD"], capture_output=True, text=True,
                             timeout=10)
    except (OSError, subprocess.SubprocessError):
        return ""
    return out.stdout.strip() if out.returncode == 0 else ""


def docker_lists_ours(words, bodies):
    """A docker listing (`docker ps -aq --filter name=kinige-`) whose filter names our objects."""
    if not words or os.path.basename(words[0]) != "docker" or not DOCKER_LISTING & set(words[1:]):
        return False
    args = words[1:]
    filters = [bodies.get(args[i + 1], args[i + 1]) for i, a in enumerate(args[:-1]) if a in ("--filter", "-f")]
    filters += [a.split("=", 1)[1] for a in args if a.startswith("--filter=")]
    return any("kinige" in f for f in filters)


def check_docker(name, args, listing_ours):
    if name != "docker":
        return None
    if "prune" in args:
        return "`docker … prune` would remove other projects' objects on ubuntu-server. Remove your own `kinige-` objects by name."
    verbs = [a for a in args if a in DOCKER_REMOVING]
    if not verbs:
        return None
    if "compose" in args:
        names = [args[i + 1] for i, a in enumerate(args[:-1]) if a in ("-p", "--project-name")]
        ok = bool(names) and all(n.startswith("kinige") for n in names)
    else:
        targets = [a for a in args[args.index(verbs[0]) + 1:] if not a.startswith("-")]
        # No targets left: they come from a substitution or a pipe (`docker rm -f $(docker ps -aq --filter
        # name=kinige-)`), which the segment split removed. Then a docker listing in the line must filter by our
        # name; `docker rm -f $(docker ps -aq)` would remove every project's containers, and a path with "kinige" in
        # it (every checkout and scratch path) proves nothing.
        ok = all(t.startswith("kinige") for t in targets) if targets else listing_ours
    if not ok:
        return ("this docker command targets objects that are not named `kinige-…` (a compose project needs `-p kinige-…`). "
                "Touch only your own objects on ubuntu-server.")
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
        return ("only {:.1f} GiB free (the floor is {:g} GiB). Delete your own CARGO_TARGET_DIR or other files you created, "
                "or report to the coordinator; do not clean shared caches.").format(free, MIN_FREE_GIB)
    return None


def gitleaks_problem(repo, src):
    if not shutil.which("gitleaks"):
        return None
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


def in_main_checkout(path, project):
    if not (path == project or path.startswith(project + os.sep)):
        return False
    rel = os.path.relpath(path, project)
    return not (rel.startswith(WORKTREE_DIR) or rel.startswith(MEMORY_DIRS))


if __name__ == "__main__":
    sys.exit(main())

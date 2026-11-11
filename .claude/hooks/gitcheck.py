"""The git rules of `guard.py` (KINIG-110): what a subagent may run in the main checkout, and which pushes go out.

A subagent works in its own worktree (CLAUDE.md rule 13), so in the main checkout it may run only git that changes
nothing there. A push never goes to main, never forces, and never carries a commit in which gitleaks finds a secret
(the repository is public).
"""

import os
import re
import shutil
import subprocess

import shellparse

# What a subagent may run in the main checkout: git that does not change it. Everything else is blocked there.
GIT_READ_ONLY = {
    "status", "log", "diff", "show", "fetch", "rev-parse", "ls-files", "ls-tree", "blame", "grep", "merge-base",
    "cat-file", "describe", "shortlog", "ls-remote", "for-each-ref", "rev-list", "name-rev", "check-ignore", "version",
    "help", "push",
}
PUSH_OPTIONS_WITH_VALUE = {"-o", "--push-option", "--repo", "--receive-pack", "--exec"}
WORKTREE_DIR = os.path.join(".claude", "worktrees")
MEMORY_DIRS = (os.path.join(".claude", "agent-memory"), os.path.join(".claude", "agent-memory-local"))


def check_git(name, args, data, workdir):
    if name != "git":
        return None
    sub, rest, repo = shellparse.git_subcommand(args, workdir)
    # A directory named by a variable the line does not assign (`cd "$WT"` with WT set in an earlier call) is unknown
    # here, so it is not judged as the main checkout.
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


def in_main_checkout(path, project):
    if not (path == project or path.startswith(project + os.sep)):
        return False
    rel = os.path.relpath(path, project)
    return not (rel.startswith(WORKTREE_DIR) or rel.startswith(MEMORY_DIRS))


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

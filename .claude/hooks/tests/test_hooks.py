"""Tests of `.claude/hooks/guard.py` (KINIG-110). Run: `python3 -I .claude/hooks/tests/test_hooks.py` (CI, the `web` job).

Every case names the exact exit code it wants (0 lets the call run, 2 blocks or reports), so a guard that crashed or
failed for another reason does not count as having caught anything: an internal error exits 0 with a note, which the
"blocked" cases would see as a failure. Each case is its own subtest, so a do-nothing guard fails every one of them.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid

HOOKS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GUARD = os.path.join(HOOKS, "guard.py")
HOME = os.path.expanduser("~")

# Run with the cwd in a throwaway Kinige-shaped repository on branch `main`.
BLOCKED_COMMANDS = [
    "sed -i 's/a/b/' crates/x.rs",
    "sed -E -i '' 's/a/b/' web/src/a.ts",
    "cd web && sed -i.bak s/a/b/ src/a.ts",
    "sed --in-place 's/a/b/' server/src/lib.rs",
    "\\sed -i 's/a/b/' crates/x.rs",
    "perl -pi -e 's/a/b/' crates/x.rs",
    "perl -0777 -i -pe 's/a/b/' crates/x.rs",
    "find . -name '*.rs' -exec sed -i 's/a/b/' {} +",
    "grep -rl foo crates | xargs sed -i 's/foo/bar/'",
    "bash -c 'sed -i s/a/b/ crates/x.rs'",
    "cat > crates/kinige-store/src/x.rs <<'EOF'\nfn main() {}\nEOF",
    "cat >> CLAUDE.md <<EOF\nline\nEOF",
    "printf 'x' > ./web/src/a.ts",
    "echo hi | tee -a docs/x.md",
    "echo hi | tee /tmp/a.txt crates/x.rs",
    "echo x 2>/tmp/e >crates/x.rs",
    "echo x >| crates/x.rs",
    "cd crates && echo z > x.rs",
    "python3 - <<'EOF'\nimport pathlib\np = pathlib.Path('crates/x.rs')\np.write_text(p.read_text().replace('a', 'b'))\nEOF",
    "python3 -c \"open('web/src/a.ts', 'w').write('x')\"",
    "python3.12 -c \"open('crates/a.rs', 'w').write('x')\"",
    "node -e \"require('fs').writeFileSync('web/src/a.ts', 'x')\"",
    "cat ~/Library/Application\\ Support/me.savvinov.kinige/kinige.db",
    "sqlite3 \"$HOME/Library/Application Support/me.savvinov.kinige/kinige.db\" .tables",
    "ls ~/Documents/Kinige",
    "tail -f /Users/someone/Library/Logs/me.savvinov.kinige/kinige.log",
    "KINIGE_DATA_DIR=~/Library/Application\\ Support/me.savvinov.kinige cargo run",
    "python3 - <<'EOF'\nimport os\nprint(open(os.path.expanduser('~/Documents/Kinige/x.md')).read())\nEOF",
    "open /tmp/build/Kinige.app",
    "open -a Kinige",
    "open /tmp/a.png",
    "open https://example.com",
    "killall Dock",
    "scripts/run-app.sh",
    "bash scripts/screenshot-window.sh out.png",
    "osascript -e 'tell application \"Finder\" to activate'",
    "screencapture -x /tmp/a.png",
    "tccutil reset All",
    "defaults write com.apple.dock autohide -bool true",
    "claude mcp add kinige /x/kinige-mcp",
    "echo '{}' > ~/.claude/settings.json",
    "cp /tmp/s.json ~/.claude/settings.json",
    "python3 -c \"open('/Users/x/.claude/settings.json', 'w').write('{}')\"",
    "git push origin main",
    "git push origin HEAD:main",
    "git push",
    "git push origin HEAD",
    "git push origin +fix:main",
    "git push origin +feat",
    "git push --force origin tools/x",
    "git push -f",
    "git push -uf origin tools/x",
    "git push origin --delete main",
    "git push --all origin",
    "docker system prune -af",
    "docker volume prune -f",
    "docker rm -f portainer",
    "docker volume rm other-data && docker ps | grep kinige",
    "docker compose -p other-project down -v",
    "docker compose down",
]

ALLOWED_COMMANDS = [
    "sed -n '1,20p' crates/x.rs",
    "sed 's/a/b/' crates/x.rs > /tmp/out.rs",
    "grep -rn 'sed -i' .claude/hooks",
    "git commit -m 'KINIG-110: block sed -i -> crates/x.rs'",
    "git commit -F - <<'EOF'\nKINIG-110: cat > crates/x.rs, sed -i and ~/Documents/Kinige are blocked\nEOF",
    "gh pr create --title t --body \"$(cat <<'EOF'\nsed -i and > crates/x.rs and ~/Documents/Kinige\nEOF\n)\"",
    "cargo test -p kinige-store 2>&1 | tail -5",
    "cargo test > /tmp/log.txt 2>&1",
    "cargo test # > crates/x.rs",
    "echo x > /tmp/kinige-guard-elsewhere/docs/a.json",
    "python3 /tmp/analysis.py > /tmp/out.json",
    "python3 -c 'import json; print(1)'",
    "python3 - <<'EOF'\nimport json\nprint(json.load(open('/tmp/x.json'))['a'])\nEOF",
    "perl -Mstrict -e 'print 1'",
    "cat <<'EOF' > /tmp/probe.diff\n--- a/crates/x.rs\n+++ b/crates/x.rs\nEOF",
    "git apply /tmp/probe.diff",
    "git push --force-with-lease origin fix/main-window",
    "git push -u origin tools/harness-hooks",
    "git push -o ci.skip origin tools/x",
    "docker rm -f kinige-s4c-pg-x",
    "docker rm -f $(docker ps -aq --filter name=kinige-)",
    "docker compose -p kinige-s4c2-smoke down -v",
    "docker ps -a",
    "grep -rn 'Documents/Kinige' crates",
    "echo ok >/dev/null 2>&1",
    "ls -la",
]

# Run by a subagent with its cwd in the main checkout (CLAUDE_PROJECT_DIR = the repository).
SUBAGENT_BLOCKED_GIT = [
    "git checkout -b tools/x", "git clean -fdx", "git restore crates/x.rs", "git add -A", "git branch -D foo",
    "git worktree remove --force .claude/worktrees/x", "git stash", "git reset --hard", "git commit -m x",
]
SUBAGENT_ALLOWED_GIT = [
    "git status", "git log --oneline -3", "git diff", "git fetch -q", "git branch --show-current",
    "git worktree add /tmp/kinige-wt-test -b tools/y", "git worktree list", "git stash list", "git remote -v",
]


def run(mode, payload, env=None):
    proc = subprocess.run(
        [sys.executable, "-I", GUARD, mode], input=payload if isinstance(payload, str) else json.dumps(payload),
        capture_output=True, text=True, timeout=120, env=env,
    )
    return proc.returncode, proc.stderr


def bash(command, cwd=None, agent_type=None):
    data = {"session_id": "test-" + uuid.uuid4().hex, "tool_name": "Bash", "tool_input": {"command": command},
            "tool_use_id": uuid.uuid4().hex, "cwd": cwd or tempfile.gettempdir()}
    if agent_type:
        data["agent_type"] = agent_type
    return data


class Repo(unittest.TestCase):
    """A throwaway Kinige-shaped repository (the marker file `crates/kinige-domain/Cargo.toml`) per test, on `main`."""

    def setUp(self):
        self.repo = os.path.realpath(tempfile.mkdtemp(prefix="kinige-guard-test-"))
        self.addCleanup(shutil.rmtree, self.repo, True)
        for rel in ("crates/kinige-domain/Cargo.toml", "crates/x.rs", "web/src/a.ts", "docs/x.md", "CLAUDE.md"):
            self.write(rel, "x\n")
        self.git("init", "-q", "-b", "main")
        self.commit("init")

    def write(self, rel, text):
        path = os.path.join(self.repo, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)

    def git(self, *args):
        subprocess.run(["git", "-C", self.repo, *args], check=True, capture_output=True)

    def commit(self, message):
        self.git("add", "-A")
        self.git("-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-qm", message)

    def env(self):
        return dict(os.environ, CLAUDE_PROJECT_DIR=self.repo)


class BashCommands(Repo):
    def test_blocked(self):
        for command in BLOCKED_COMMANDS:
            with self.subTest(command=command):
                code, err = run("bash", bash(command, cwd=self.repo))
                self.assertEqual(code, 2, err)
                self.assertIn("Blocked by .claude/hooks/guard.py", err)

    def test_allowed(self):
        for command in ALLOWED_COMMANDS:
            with self.subTest(command=command):
                code, err = run("bash", bash(command, cwd=self.repo))
                self.assertEqual(code, 0, err)

    def test_garbage_input_lets_the_call_through(self):
        for payload in ("", "not json", "[]", "5", json.dumps({"tool_input": {"command": 5}}),
                        json.dumps({"tool_input": None}), json.dumps({"tool_input": "x"}), "\x00\xff"):
            for mode in ("bash", "post", "paths", "unknown"):
                with self.subTest(payload=payload, mode=mode):
                    code, err = run(mode, payload)
                    self.assertEqual(code, 0, err)

    def test_disk_floor(self):
        env = dict(os.environ, KINIGE_GUARD_MIN_FREE_GIB="1000000")
        for command, want in (("cargo test --workspace", 2), ("cargo clippy -p kinige", 2), ("cargo fmt --all --check", 0)):
            with self.subTest(command=command):
                code, err = run("bash", bash(command, cwd=self.repo), env)
                self.assertEqual(code, want, err)

    @unittest.skipUnless(shutil.which("gitleaks"), "gitleaks is not installed")
    def test_a_push_of_a_secret_is_blocked(self):
        token = "ghp_" + "Zr8Kq2Lm5Nx7Pb3Vc9Td1Wf4Hj6Ys0Ua2Eg8"  # built here so the repo's own scan does not match it
        self.git("switch", "-q", "-c", "tools/x")
        self.write("crates/config.rs", 'const T: &str = "{}";\n'.format(token))
        self.commit("leak")
        code, err = run("bash", bash("git push -u origin HEAD", cwd=self.repo))
        self.assertEqual(code, 2, err)
        self.assertIn("gitleaks", err)


class PostSnapshot(Repo):
    def around(self, command, change, cwd=None):
        data = bash(command, cwd=cwd or self.repo)
        code, err = run("bash", data)
        self.assertEqual(code, 0, err)
        change()
        return run("post", data)

    def edit_and_commit(self):
        self.write("crates/x.rs", "fn committed() {}\n")
        self.commit("script edit")

    def test_script_edits_are_reported(self):
        cases = [
            ("python3 /tmp/fix.py", lambda: self.write("crates/x.rs", "fn b() {}\n"), None),
            ("ruby /tmp/gen.rb", lambda: self.write("web/src/new.ts", "x\n"), None),
            ("python3 /tmp/fix.py && git diff --stat", lambda: self.write("crates/x.rs", "fn c() {}\n"), None),
            ("python3 /tmp/fix.py; mkdir -p x", lambda: self.write("crates/x.rs", "fn d() {}\n"), None),
            ("python3 /tmp/fix.py && git commit -qam x", self.edit_and_commit, None),
            ("cd {} && python3 /tmp/fix.py", lambda: self.write("docs/x.md", "y\n"), tempfile.gettempdir()),
        ]
        for command, change, cwd in cases:
            with self.subTest(command=command):
                code, err = self.around(command.format(self.repo), change, cwd)
                self.assertEqual(code, 2, err)
                self.assertIn("Reported by", err)

    def test_an_already_dirty_file_changed_again_is_reported(self):
        self.write("crates/x.rs", "fn dirty() {}\n")
        code, err = self.around("python3 /tmp/fix.py", lambda: self.write("crates/x.rs", "fn again() {}\n"))
        self.assertEqual(code, 2, err)
        self.assertIn("crates/x.rs", err)

    def test_writing_tools_generated_files_and_deletions_are_not_reported(self):
        cases = [
            ("cargo fmt --all", "crates/x.rs"), ("git checkout -- crates/x.rs", "crates/x.rs"),
            ("bash scripts/release.sh 1.2.3", "Cargo.toml"), ("python3 /tmp/fix.py", "web/src/api/generated/T.ts"),
            ("python3 /tmp/fix.py", "docs/screens/7/a.png"),
        ]
        for command, rel in cases:
            with self.subTest(command=command, rel=rel):
                code, err = self.around(command, lambda rel=rel: self.write(rel, uuid.uuid4().hex))
                self.assertEqual(code, 0, err)
        code, err = self.around("python3 /tmp/cleanup.py", lambda: os.remove(os.path.join(self.repo, "docs/x.md")))
        self.assertEqual(code, 0, err)

    def test_no_change_and_no_snapshot(self):
        code, err = self.around("python3 /tmp/fix.py", lambda: None)
        self.assertEqual(code, 0, err)
        code, err = run("post", bash("python3 /tmp/fix.py", cwd=self.repo))
        self.assertEqual(code, 0, err)


class MainCheckout(Repo):
    def test_a_subagent_cannot_change_the_main_checkout(self):
        for command in SUBAGENT_BLOCKED_GIT:
            with self.subTest(command=command):
                code, err = run("bash", bash(command, cwd=self.repo, agent_type="slice-implementer"), self.env())
                self.assertEqual(code, 2, err)
        with self.subTest(command="git -C <main> commit"):
            data = bash("git -C {} commit -m x".format(self.repo), agent_type="slice-reviewer")
            code, err = run("bash", data, self.env())
            self.assertEqual(code, 2, err)
        for tool, rel, agent in (("Edit", "crates/x.rs", "slice-implementer"), ("Write", "crates/y.rs", "slice-spec")):
            with self.subTest(tool=tool, rel=rel, agent=agent):
                data = {"tool_name": tool, "tool_input": {"file_path": os.path.join(self.repo, rel)}, "agent_type": agent}
                code, err = run("paths", data, self.env())
                self.assertEqual(code, 2, err)

    def test_what_stays_allowed(self):
        for command in SUBAGENT_ALLOWED_GIT:
            with self.subTest(command=command):
                code, err = run("bash", bash(command, cwd=self.repo, agent_type="slice-reviewer"), self.env())
                self.assertEqual(code, 0, err)
        cases = [
            ("Edit", ".claude/worktrees/a/crates/x.rs", "slice-implementer"),
            ("Write", ".claude/agent-memory/r/MEMORY.md", "slice-reviewer"),
            ("Write", "docs/slices/11-sync.md", "slice-spec"),
            ("Edit", "crates/x.rs", None),
            ("Read", "crates/x.rs", "slice-reviewer"),
        ]
        for tool, rel, agent in cases:
            with self.subTest(tool=tool, rel=rel, agent=agent):
                data = {"tool_name": tool, "tool_input": {"file_path": os.path.join(self.repo, rel)}}
                if agent:
                    data["agent_type"] = agent
                code, err = run("paths", data, self.env())
                self.assertEqual(code, 0, err)
        code, err = run("bash", bash("git checkout -b tools/x", cwd=self.repo), self.env())
        self.assertEqual(code, 0, err)


class Paths(unittest.TestCase):
    def test_user_data_and_claude_configs(self):
        blocked = [
            ("Read", os.path.join(HOME, "Library/Application Support/me.savvinov.kinige/kinige.db")),
            ("Grep", os.path.join(HOME, "Documents/Kinige")),
            ("Edit", os.path.join(HOME, "Library/Logs/me.savvinov.kinige/kinige.log")),
            ("Write", os.path.join(HOME, ".claude/settings.json")),
            ("Edit", "~/.claude.json"),
        ]
        for tool, path in blocked:
            with self.subTest(tool=tool, path=path):
                key = "path" if tool == "Grep" else "file_path"
                code, err = run("paths", {"tool_name": tool, "tool_input": {key: path}})
                self.assertEqual(code, 2, err)
        allowed = [
            ("Read", os.path.join(HOME, ".claude/settings.json")),
            ("Read", os.path.join(HOME, "Documents/Kinige-other/x")),
            ("Write", os.path.join(tempfile.gettempdir(), "kinige-data", "x.db")),
        ]
        for tool, path in allowed:
            with self.subTest(tool=tool, path=path):
                code, err = run("paths", {"tool_name": tool, "tool_input": {"file_path": path}})
                self.assertEqual(code, 0, err)


if __name__ == "__main__":
    unittest.main(verbosity=1)

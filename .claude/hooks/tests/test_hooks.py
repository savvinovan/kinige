"""Tests of `.claude/hooks/guard.py` (KINIG-110). Run: `python3 -I .claude/hooks/tests/test_hooks.py` (CI, the `web` job).

Every case names the exact exit code it wants (0 lets the call run, 2 blocks or reports), so a guard that crashed or
failed for another reason does not count as having caught anything: an internal error exits 0 with a note, which the
"blocked" cases would see as a failure.
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

BLOCKED_COMMANDS = [
    "sed -i 's/a/b/' crates/x.rs",
    "sed -E -i '' 's/a/b/' web/src/a.ts",
    "cd web && sed -i.bak s/a/b/ src/a.ts",
    "sed --in-place 's/a/b/' server/src/lib.rs",
    "perl -pi -e 's/a/b/' crates/x.rs",
    "perl -0777 -i -pe 's/a/b/' crates/x.rs",
    "find . -name '*.rs' -exec sed -i 's/a/b/' {} +",
    "grep -rl foo crates | xargs sed -i 's/foo/bar/'",
    "bash -c 'sed -i s/a/b/ crates/x.rs'",
    "cat > crates/kinige-store/src/x.rs <<'EOF'\nfn main() {}\nEOF",
    "cat >> CLAUDE.md <<EOF\nline\nEOF",
    "printf 'x' > ./web/src/a.ts",
    "echo hi | tee -a docs/x.md",
    "python3 - <<'EOF'\nimport pathlib\np = pathlib.Path('crates/x.rs')\np.write_text(p.read_text().replace('a', 'b'))\nEOF",
    "python3 -c \"open('web/src/a.ts', 'w').write('x')\"",
    "python3.12 -c \"open('crates/a.rs', 'w').write('x')\"",
    "node -e \"require('fs').writeFileSync('web/src/a.ts', 'x')\"",
    "cat ~/Library/Application\\ Support/me.savvinov.kinige/kinige.db",
    "sqlite3 \"$HOME/Library/Application Support/me.savvinov.kinige/kinige.db\" .tables",
    "ls ~/Documents/Kinige",
    "tail -f /Users/someone/Library/Logs/me.savvinov.kinige/kinige.log",
    "python3 - <<'EOF'\nimport os\nprint(open(os.path.expanduser('~/Documents/Kinige/x.md')).read())\nEOF",
    "open /tmp/build/Kinige.app",
    "open -a Kinige",
    "scripts/run-app.sh",
    "bash scripts/screenshot-window.sh out.png",
    "osascript -e 'tell application \"Finder\" to activate'",
    "screencapture -x /tmp/a.png",
    "tccutil reset All",
    "defaults write com.apple.dock autohide -bool true",
    "claude mcp add kinige /x/kinige-mcp",
    "echo '{}' > ~/.claude/settings.json",
    "git push origin main",
    "git push origin HEAD:main",
    "git push origin +fix:main",
    "git push --force origin tools/x",
    "git push -f",
    "git push -uf origin tools/x",
    "git push origin --delete main",
    "git push --all origin",
    "docker system prune -af",
    "docker volume prune -f",
    "docker rm -f portainer",
    "docker compose -p other-project down -v",
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
    "python3 /tmp/analysis.py > /tmp/out.json",
    "python3 -c 'import json; print(1)'",
    "python3 - <<'EOF'\nimport json\nprint(json.load(open('/tmp/x.json'))['a'])\nEOF",
    "perl -Mstrict -e 'print 1'",
    "cat <<'EOF' > /tmp/probe.diff\n--- a/crates/x.rs\n+++ b/crates/x.rs\nEOF",
    "git apply /tmp/probe.diff",
    "git push --force-with-lease origin fix/main-window",
    "git push -u origin tools/harness-hooks",
    "docker rm -f kinige-s4c-pg-x",
    "docker rm -f $(docker ps -aq --filter name=kinige-)",
    "docker ps -a",
    "grep -rn 'Documents/Kinige' crates",
    "open https://example.com",
    "echo ok >/dev/null 2>&1",
    "ls -la",
]


def run(mode, payload, env=None):
    proc = subprocess.run(
        [sys.executable, "-I", GUARD, mode], input=payload if isinstance(payload, str) else json.dumps(payload),
        capture_output=True, text=True, timeout=120, env=env,
    )
    return proc.returncode, proc.stderr


def bash(command, cwd=None, agent_type=None, tool_use_id=None):
    data = {"session_id": "test-" + uuid.uuid4().hex, "tool_name": "Bash", "tool_input": {"command": command},
            "tool_use_id": tool_use_id or uuid.uuid4().hex, "cwd": cwd or tempfile.gettempdir()}
    if agent_type:
        data["agent_type"] = agent_type
    return data


class BashCommands(unittest.TestCase):
    def test_blocked(self):
        for command in BLOCKED_COMMANDS:
            with self.subTest(command=command):
                code, err = run("bash", bash(command))
                self.assertEqual(code, 2, err)
                self.assertIn("Blocked by .claude/hooks/guard.py", err)

    def test_allowed(self):
        for command in ALLOWED_COMMANDS:
            with self.subTest(command=command):
                code, err = run("bash", bash(command))
                self.assertEqual(code, 0, err)

    def test_garbage_input_lets_the_call_through(self):
        for payload in ("", "not json", "[]", "5", json.dumps({"tool_input": {"command": 5}}),
                        json.dumps({"tool_input": None}), "\x00\xff"):
            for mode in ("bash", "post", "paths", "unknown"):
                with self.subTest(payload=payload, mode=mode):
                    code, err = run(mode, payload)
                    self.assertEqual(code, 0, err)

    def test_disk_floor(self):
        env = dict(os.environ, KINIGE_GUARD_MIN_FREE_GIB="1000000")
        code, err = run("bash", bash("cargo test --workspace"), env)
        self.assertEqual(code, 2, err)
        self.assertIn("GiB free", err)
        code, err = run("bash", bash("cargo fmt --all --check"), env)
        self.assertEqual(code, 0, err)


class Repos(unittest.TestCase):
    """A throwaway Kinige-shaped repository (the marker file `crates/kinige-domain/Cargo.toml`) per test."""

    def setUp(self):
        self.repo = os.path.realpath(tempfile.mkdtemp(prefix="kinige-guard-test-"))
        self.addCleanup(shutil.rmtree, self.repo, True)
        self.write("crates/kinige-domain/Cargo.toml", "[package]\n")
        self.write("crates/x.rs", "fn a() {}\n")
        self.git("init", "-q")
        self.git("add", ".")
        self.git("-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-qm", "init")

    def write(self, rel, text):
        path = os.path.join(self.repo, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)

    def git(self, *args):
        subprocess.run(["git", "-C", self.repo, *args], check=True, capture_output=True)

    def env(self):
        return dict(os.environ, CLAUDE_PROJECT_DIR=self.repo)


class PostSnapshot(Repos):
    def around(self, command, change):
        data = bash(command, cwd=self.repo)
        code, err = run("bash", data)
        self.assertEqual(code, 0, err)
        change()
        return run("post", data)

    def test_a_script_edit_of_a_source_is_reported(self):
        code, err = self.around("python3 /tmp/fix.py", lambda: self.write("crates/x.rs", "fn b() {}\n"))
        self.assertEqual(code, 2, err)
        self.assertIn("crates/x.rs", err)
        self.assertIn("Reported by", err)

    def test_a_new_source_file_is_reported(self):
        code, err = self.around("ruby /tmp/gen.rb", lambda: self.write("web/src/new.ts", "x\n"))
        self.assertEqual(code, 2, err)
        self.assertIn("web/src/new.ts", err)

    def test_an_already_dirty_file_changed_again_is_reported(self):
        self.write("crates/x.rs", "fn dirty() {}\n")
        code, err = self.around("python3 /tmp/fix.py", lambda: self.write("crates/x.rs", "fn again() {}\n"))
        self.assertEqual(code, 2, err)

    def test_writing_tools_and_generated_files_are_not_reported(self):
        for command, rel in (("cargo fmt --all", "crates/x.rs"), ("git checkout -- crates/x.rs", "crates/x.rs"),
                             ("bash scripts/release.sh 1.2.3", "Cargo.toml"),
                             ("python3 /tmp/fix.py", "web/src/api/generated/T.ts"),
                             ("python3 /tmp/fix.py", "docs/screens/7/a.png")):
            with self.subTest(command=command, rel=rel):
                code, err = self.around(command, lambda rel=rel: self.write(rel, uuid.uuid4().hex))
                self.assertEqual(code, 0, err)

    def test_no_change_and_no_snapshot(self):
        outside = tempfile.mkdtemp(prefix="kinige-guard-out-")
        self.addCleanup(shutil.rmtree, outside, True)
        code, err = self.around("python3 /tmp/fix.py", lambda: self.write(os.path.join(outside, "x.txt"), "x"))
        self.assertEqual(code, 0, err)
        code, err = run("post", bash("python3 /tmp/fix.py", cwd=self.repo))
        self.assertEqual(code, 0, err)


class MainCheckout(Repos):
    def test_a_subagent_cannot_edit_or_switch_the_main_checkout(self):
        data = {"tool_name": "Edit", "tool_input": {"file_path": os.path.join(self.repo, "crates/x.rs")},
                "agent_type": "slice-implementer"}
        code, err = run("paths", data, self.env())
        self.assertEqual(code, 2, err)
        code, err = run("bash", bash("git checkout -b tools/x", cwd=self.repo, agent_type="slice-implementer"), self.env())
        self.assertEqual(code, 2, err)
        code, err = run("bash", bash("git -C {} commit -m x".format(self.repo), agent_type="slice-reviewer"), self.env())
        self.assertEqual(code, 2, err)

    def test_worktrees_memory_and_the_coordinator_are_allowed(self):
        cases = [
            ({"tool_name": "Edit", "tool_input": {"file_path": os.path.join(self.repo, ".claude/worktrees/a/crates/x.rs")},
              "agent_type": "slice-implementer"}, "paths"),
            ({"tool_name": "Write", "tool_input": {"file_path": os.path.join(self.repo, ".claude/agent-memory/r/MEMORY.md")},
              "agent_type": "slice-reviewer"}, "paths"),
            ({"tool_name": "Edit", "tool_input": {"file_path": os.path.join(self.repo, "crates/x.rs")}}, "paths"),
            ({"tool_name": "Read", "tool_input": {"file_path": os.path.join(self.repo, "crates/x.rs")},
              "agent_type": "slice-reviewer"}, "paths"),
            (bash("git status", cwd=self.repo, agent_type="slice-reviewer"), "bash"),
            (bash("git checkout -b tools/x", cwd=self.repo), "bash"),
        ]
        for data, mode in cases:
            with self.subTest(data=data):
                code, err = run(mode, data, self.env())
                self.assertEqual(code, 0, err)

    @unittest.skipUnless(shutil.which("gitleaks"), "gitleaks is not installed")
    def test_a_push_of_a_secret_is_blocked(self):
        token = "ghp_" + "Zr8Kq2Lm5Nx7Pb3Vc9Td1Wf4Hj6Ys0Ua2Eg8"  # built here so the repo's own scan does not match it
        self.write("crates/config.rs", 'const T: &str = "{}";\n'.format(token))
        self.git("add", ".")
        self.git("-c", "user.email=t@example.invalid", "-c", "user.name=t", "commit", "-qm", "leak")
        code, err = run("bash", bash("git push -u origin HEAD", cwd=self.repo), self.env())
        self.assertEqual(code, 2, err)
        self.assertIn("gitleaks", err)


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

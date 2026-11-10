"""Controls of scripts/pre-pr-check.py (KINIG-110). Run: python3 -I scripts/pre-pr-check.test.py (CI, the `web` job).

Each case runs the check in a throwaway repository whose `origin/main` is its first commit, and names the exact exit
code it wants (0 clean, 1 findings, 2 could not run; in `--hook` mode 2 means findings and 0 everything else).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

CHECK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "pre-pr-check.py")
ZWSP = chr(0x200B)  # code points, so this file stays plain ASCII and passes the check itself
ZWJ = chr(0x200D)
MAN, WOMAN = chr(0x1F468), chr(0x1F469)
OLD_TABLE = "| a | b |\n|--|--|\n| long cell | x |\n"


class PrePrCheck(unittest.TestCase):
    def setUp(self):
        self.repo = tempfile.mkdtemp(prefix="kinige-prcheck-")
        self.addCleanup(shutil.rmtree, self.repo, True)
        self.write("crates/big.rs", "x\n" * 450)
        self.write("docs/old.md", "# Old\n\n" + OLD_TABLE + "\ntail\n")
        self.git("init", "-q")
        self.commit("base")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

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

    def check(self, *args, stdin=None):
        proc = subprocess.run([sys.executable, "-I", CHECK, *args], cwd=self.repo, input=stdin,
                              capture_output=True, text=True, timeout=120)
        return proc.returncode, proc.stdout + proc.stderr

    def test_clean_branch(self):
        self.write("crates/small.rs", "fn a() {}\n")
        cyrillic = "".join(map(chr, (0x0421, 0x043B, 0x0430, 0x0439, 0x0441)))  # a five-letter word, one column wide each
        self.write("docs/new.md", "| {} | b |\n|-------|---|\n| 7a    | x |\n".format(cyrillic))
        self.write("docs/emoji.md", "family: " + MAN + ZWJ + WOMAN + "\n")
        self.commit("change")
        code, out = self.check()
        self.assertEqual(code, 0, out)

    def test_findings(self):
        cases = [
            ("crates/long.rs", "x\n" * 401, "401 lines"),
            ("crates/a.rs.bak", "x\n", "backup file"),
            ("docs/inv.md", "text" + ZWSP + "here\n", "U+200B"),
            ("docs/table.md", "| a | b |\n|---|---|\n| longer | x |\n", "not aligned"),
            ("docs/ws.md", "trailing   \n", "whitespace"),
            ("crates/merge.rs", "a\n<<<<<<< HEAD\nb\n", "conflict marker"),
        ]
        for rel, text, expect in cases:
            with self.subTest(rel=rel):
                self.write(rel, text)
                code, out = self.check()
                self.assertEqual(code, 1, out)
                self.assertIn(expect, out)
                os.remove(os.path.join(self.repo, rel))

    def test_grandfathered_and_allowed(self):
        self.write("crates/big.rs", "x\n" * 470)
        self.write("docs/old.md", "# Old, edited heading\n\n" + OLD_TABLE + "\ntail\n")
        self.write("crates/long.rs", "x\n" * 420)
        self.write("scripts/pre-pr-check.allow", "crates/long.rs  # one generated table, split in KINIG-1\n")
        self.write("crates/kinige-store/migrations/0009.sql", "x\n" * 900)
        code, out = self.check()
        self.assertEqual(code, 0, out)

    def test_hook_mode(self):
        self.write("crates/long.rs", "x\n" * 401)
        payload = json.dumps({"cwd": self.repo, "stop_hook_active": False})
        code, out = self.check("--hook", stdin=payload)
        self.assertEqual(code, 2, out)
        self.assertIn("401 lines", out)
        for stdin in (json.dumps({"cwd": self.repo, "stop_hook_active": True}), "garbage", "[]",
                      json.dumps({"cwd": tempfile.gettempdir()})):
            with self.subTest(stdin=stdin):
                code, out = self.check("--hook", stdin=stdin)
                self.assertEqual(code, 0, out)

    def test_cannot_run(self):
        code, out = self.check("--base", "refs/remotes/origin/nothing")
        self.assertEqual(code, 2, out)


if __name__ == "__main__":
    unittest.main(verbosity=1)

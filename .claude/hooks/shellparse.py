"""A small, conservative reader of Bash command lines for `guard.py` (KINIG-110).

It is not a shell parser. It finds the simple commands of a line well enough to check their programs, arguments and
redirects; what it cannot see (`eval`, a script piped into `sh`, variables) is left to the snapshot check after the
command (`snapshot.py`).
"""

import os
import re

PREFIX_WORDS = ("sudo", "time", "command", "exec", "env", "nohup", "builtin")


def skeleton(command):
    """The command without heredoc bodies and comments, and its quoted strings.

    A quoted string becomes its text when it is one word (a path, a sed script), otherwise a `QSTRn` placeholder whose
    text is in the returned dict (for `bash -c`). Commit messages and PR bodies are quoted or heredocs, so an arrow or a
    path inside them is never read as a redirect.
    """
    text = re.sub(r"<<-?\s*\\?(['\"]?)(\w[\w-]*)\1([^\n]*)\n.*?\n\s*\2[ \t]*(?=\n|$)", r"<< \3", command, flags=re.S)
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
            # A one-word body goes back as text (a path, a sed script); one with shell metacharacters stays opaque, so
            # `python3 -c 'print(open("x").read())'` is not read as a command `open`.
            if body and not re.search(r"[\s()|;&<>`]", body):
                out.append(body)
            else:
                key = "QSTR{}".format(len(bodies))
                bodies[key] = body
                out.append(key)
            i = j + 1
            continue
        if c == "#" and (i == 0 or text[i - 1] in " \t\n;&|("):
            j = text.find("\n", i)
            i = len(text) if j < 0 else j
            continue
        out.append(c)
        i += 1
    return "".join(out), bodies


def segments(skel):
    """The raw words of each simple command, split on `&&`, `||`, `;`, `|`, `(`, `)`, `$(` and newlines."""
    skel = skel.replace(">|", "> ")  # the clobber redirect is a redirect, not a pipe
    return [re.findall(r"\d?&?>>?\S*|<<|\S+", part) for part in re.split(r"&&|\|\||[;|\n()`]|\$\(", skel)]


def command_words(words):
    """The words from the program on: leading `VAR=value` assignments and prefixes like `sudo` dropped, and the
    command that `xargs`, `find -exec`, `nice` or `timeout` run unwrapped, so `xargs sed -i` reads as `sed -i`."""
    while words and (re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0]) or words[0].lstrip("\\") in PREFIX_WORDS):
        words = words[1:]
    if words and words[0].startswith("\\"):
        words = [words[0].lstrip("\\")] + words[1:]
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


def redirect_targets(words):
    out = []
    for i, w in enumerate(words):
        m = re.match(r"^\d?(?:>>?|&>>?)(.*)$", w)
        if not m:
            continue
        target = m.group(1) or (words[i + 1] if i + 1 < len(words) else "")
        if target and not target.startswith("&") and target != "/dev/null":
            out.append(target)
    return out


def tee_targets(name, args):
    return [a for a in args if not a.startswith("-")] if name == "tee" else []


def git_subcommand(args, workdir):
    """(subcommand, its arguments, the repository directory) of `git <args>`, following `-C`."""
    repo, i = workdir, 0
    while i < len(args) and args[i].startswith("-"):
        if args[i] == "-C" and i + 1 < len(args):
            repo = os.path.normpath(os.path.join(repo, os.path.expanduser(args[i + 1])))
            i += 2
        elif args[i] in ("-c", "--git-dir", "--work-tree", "--namespace") and i + 1 < len(args):
            i += 2
        else:
            i += 1
    if i >= len(args):
        return "", [], repo
    return args[i], args[i + 1:], repo

"""Command lines for `test_hooks.py` about redirects (KINIG-120), kept apart so that file stays under 400 lines.

A redirect is not an argument: `docker rm -f kinige-x >/dev/null` removes one object, ours. The same words must not hide
a foreign target, make a removal with no targets look like one that is ours, hide the destination of a copy, or hide
the branch of a push. Like the lists in `test_hooks.py`, they run with the cwd in a throwaway repository on `main`.
"""

REDIRECT_BLOCKED = [
    "docker rm -f portainer >/dev/null",
    "docker rm -f portainer 2>&1",
    "docker rm -f kinige-x portainer > /dev/null",
    "2>/dev/null docker rm -f portainer",
    # No targets: a removal the redirect words must not turn into one that is allowed.
    "docker rm -f >/dev/null",
    "docker rm -f > /dev/null",
    "docker rm -f 2>/dev/null",
    "docker rm -f </dev/null",
    "docker help >/dev/null; docker rm -f portainer",
    "docker rm -f kinige-x '>' portainer",
    # A bare operator that ends its segment is kept, so the words after the substitution are not lost.
    "docker rm -f kinige-x <(true) portainer",
    "docker rm -f kinige-x >(cat) portainer",
    "docker rm -f kinige-x > $(echo f) portainer",
    "docker rm -f kinige-x < `echo f` portainer",
    # A prefix word after a leading redirect is still unwrapped.
    ">/dev/null sudo docker rm -f portainer",
    "2>/dev/null X=1 docker rm -f portainer",
    "2>/dev/null env git push origin main",
    ">/dev/null command git push --force origin tools/x",
    "2>/dev/null env cp /tmp/s.json ~/.claude/settings.json",
    # `--help` exempts only a line that names nothing to remove.
    "docker rm -f portainer & docker rm --help",
    "docker rm --help & docker rm -f portainer",
    "docker help & docker rm -f portainer",
    "docker rm -f portainer &! docker rm --help",
    "docker rm -f -- portainer --help",
    "docker rm -f kinige-x -- --help portainer",
    "echo portainer | xargs docker rm -f -- --help",
    "docker help rm",
    # A word that a shell may split into `--` and a name is not a flag.
    "X='- portainer'; docker rm -f -$X --help",
    "docker rm -f -$X --help",
    "docker rm -f --help -$X",
    # zsh, the Bash tool's shell: a descriptor has one digit, so `12345` is a container ID; `<->` is a glob.
    "docker rm -f kinige-x 12345>/dev/null",
    "docker rm -f kinige-x 10>/dev/null",
    "docker rm -f kinige-x 99>&2",
    "docker rm -f kinige-x 12<>/tmp/w",
    "docker rm -f kinige-x <->",
    "docker rm -f kinige-x <1-9>",
    "docker rm -f $(docker ps -aq) >/dev/null",
    "docker rm -f $(docker ps -aq) 2>&1 | tail -1",
    # A redirect into a source file is still the shell write of rule 8.
    "docker rm -f kinige-x > crates/x.rs",
    "docker rm -f kinige-x >crates/x.rs",
    "docker rm -f kinige-x &>crates/x.rs",
    "docker rm -f kinige-x >>crates/x.rs",
    # Other checks that read arguments: the destination of a copy, the branch of a push (the repository is on main).
    "cp /tmp/s.json ~/.claude/settings.json 2>&1",
    "cp /tmp/s.json ~/.claude/settings.json >/dev/null",
    "git push origin 2>&1",
    "git push origin >/dev/null",
    # The rarer operators: `>&`, `<>`, `<&`, a descriptor of two digits or a named one.
    "git push origin >& /dev/null",
    "git push origin <> /dev/null",
    "git push origin <& 0",
    "git push origin 10>/dev/null",
    "git push origin {fd}>/dev/null",
    "cp /tmp/s.json ~/.claude/settings.json >& /dev/null",
    "cp /tmp/s.json ~/.claude/settings.json 2>& /dev/null",
    "cp /tmp/s.json ~/.claude/settings.json <> /dev/null",
    "cp /tmp/s.json ~/.claude/settings.json 10>/dev/null",
    "cp /tmp/s.json ~/.claude/settings.json {fd}>/dev/null",
]

REDIRECT_ALLOWED = [
    "docker rm -f kinige-x >/dev/null && echo ok",
    "docker rm -f kinige-x > /dev/null && echo ok",
    "docker rm -f kinige-x 2>/dev/null",
    "docker rm -f kinige-x 2>&1",
    "docker rm -f kinige-x &>/dev/null",
    "docker rm -f kinige-x >>/tmp/rm.log",
    "docker rm -f kinige-x </dev/null",
    "docker rm -f kinige-x < /dev/null",
    "docker rm -f -v kinige-x kinige-y >/dev/null 2>&1 || true",
    "docker rm -f kinige-x 2>&1 | tail -1",
    "docker rm -f $(docker ps -aq --filter name=kinige- 2>/dev/null) >/dev/null",
    "docker ps -aq --filter name=kinige- 2>/dev/null | xargs docker rm -f >/dev/null",
    # The usage text removes nothing.
    "docker rm --help",
    "docker rm --help 2>&1 | grep -i volume",
    "docker rm -f kinige-x >& /dev/null",
    "docker rm -f kinige-x {fd}>/dev/null",
    "2>/dev/null docker rm -f kinige-x",
    "cp /tmp/s.json /tmp/t.json 2>&1",
    "git push -u origin tools/x >/dev/null 2>&1",
]

#!/usr/bin/env python3
"""Fail when a workflow mints a GitHub App token wider than its hop needs.

The fts-forge App is installed org-wide with broad permissions, so every
token is narrowed when it is minted (actions/create-github-app-token):
exactly ONE repository, and only the permissions listed for that workflow
and job below. This checks every `.github/workflows/*.yml`:

  - each mint names `repositories:` with exactly one repository, and that
    repository and every `permission-*` it asks for are allowed for that
    workflow's job in ALLOWED;
  - no mint asks for `workflows`, `administration`, `organization-*` or
    any other permission outside the list;
  - no job exports a minted token as a job output.

A mint in a workflow or job not listed fails: add it here, with the
reason, in the same PR. Run by pr.yml (and `just check-token-mints`).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# workflow file -> job id -> (repository, {permission: level})
ALLOWED: dict[str, dict[str, tuple[str, dict[str, str]]]] = {
    "bump-architect.yml": {
        # push bump/architect-<tag>, open the PR, arm auto-merge
        "open": ("keyflow", {"contents": "write", "pull-requests": "write"}),
    },
}

MINT = re.compile(r"^(\s*)-?\s*uses:\s*actions/create-github-app-token@")


def mints(text: str):
    """(job id, line number, {key: value}) for every token mint."""
    lines = text.splitlines()
    job = None
    in_jobs = False
    for i, line in enumerate(lines):
        if re.match(r"^jobs:\s*$", line):
            in_jobs = True
            continue
        m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if in_jobs and m:
            job = m.group(1)
        if not MINT.match(line):
            continue
        # The step's keys: every following line indented deeper than the
        # step's `- ` marker, until the next step or a shallower line.
        step_indent = len(line) - len(line.lstrip(" -"))
        for back in range(i, max(i - 6, 0) - 1, -1):
            if lines[back].lstrip().startswith("- "):
                step_indent = len(lines[back]) - len(lines[back].lstrip())
                break
        keys: dict[str, str] = {}
        for nxt in lines[i + 1:]:
            if not nxt.strip():
                continue
            indent = len(nxt) - len(nxt.lstrip())
            if indent <= step_indent:
                break
            kv = re.match(r"^\s*([A-Za-z0-9_-]+):\s*(.*?)\s*$", nxt)
            if kv:
                keys[kv.group(1)] = kv.group(2).strip("'\"")
        yield job, i + 1, keys


def main(root: Path = ROOT) -> int:
    bad: list[str] = []
    for path in sorted((root / ".github" / "workflows").glob("*.y*ml")):
        text = path.read_text()
        name = path.name
        for m in re.finditer(r"^    outputs:\s*\n((?:      .*\n)+)", text, re.M):
            if ".outputs.token" in m.group(1):
                bad.append(f"{name}: a job exports a minted token as an output")
        for n, line in enumerate(text.splitlines(), 1):
            if re.search(r"(outputs\.token|\$\{?GH_TOKEN|\$\{?TOKEN)\b.*>>\s*\"?\$\{?GITHUB_(OUTPUT|ENV|STEP_SUMMARY)", line):
                bad.append(f"{name}:{n}: a token written to $GITHUB_OUTPUT/ENV/STEP_SUMMARY")
        for job, line, keys in mints(text):
            where = f"{name}:{line} (job {job})"
            allowed = ALLOWED.get(name, {}).get(job or "")
            if allowed is None:
                bad.append(f"{where}: a token mint not in scripts/check-token-mints.py's ALLOWED")
                continue
            repo, perms = allowed
            repos = [r.strip() for r in re.split(r"[,\s]+", keys.get("repositories", "")) if r.strip()]
            if repos != [repo]:
                bad.append(f"{where}: repositories must be exactly '{repo}', is {repos or 'missing'}")
            asked = {k[len("permission-"):]: v for k, v in keys.items() if k.startswith("permission-")}
            if not asked:
                bad.append(f"{where}: no permission-* given (the token would get every permission the App has)")
            for perm, level in sorted(asked.items()):
                if perm not in perms:
                    bad.append(f"{where}: permission-{perm} is not allowed here")
                elif level != perms[perm] and not (perms[perm] == "write" and level == "read"):
                    bad.append(f"{where}: permission-{perm}: {level}, allowed {perms[perm]}")
            if keys.get("owner") not in ("FastTrackStudios", None):
                bad.append(f"{where}: owner must be FastTrackStudios")
    if bad:
        print("token mints wider than their hop needs:")
        print("\n".join(f"  {b}" for b in bad))
        return 1
    print("ok: every App token is minted for one repository with its listed permissions")
    return 0


def self_test() -> int:
    """A wider mint, a second repository and an exported token must fail."""
    import tempfile
    wide = """jobs:
  open:
    outputs:
      t: ${{ steps.forge.outputs.token }}
    steps:
      - uses: actions/create-github-app-token@v2
        id: forge
        with:
          repositories: a,b
          permission-workflows: write
      - run: echo "$GH_TOKEN" >> "$GITHUB_OUTPUT"
"""
    with tempfile.TemporaryDirectory() as d:
        wf = Path(d) / ".github" / "workflows"
        wf.mkdir(parents=True)
        for name in ALLOWED:
            (wf / name).write_text(wide)
        if main(Path(d)) == 0:
            print("self-test FAILED: a wide mint passed")
            return 1
    print("self-test ok: a wide mint fails")
    return 0


if __name__ == "__main__":
    sys.exit(self_test() if "--self-test" in sys.argv else main())

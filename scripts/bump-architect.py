#!/usr/bin/env python3
"""Move every Architect pin in this repo to one new release tag.

    just bump-architect v0.12.5                  # the pins, the comments, Cargo.lock
    just bump-architect v0.12.5 --version 0.5.4  # and the workspace version (a keyflow release)
    just bump-architect v0.13.0 --version auto   # the next patch version: what a MINOR needs

An Architect PATCH release needs no keyflow release: Studio patches this
repo's (www-spelled) Architect pins to its own tag. A MINOR does: bump with
`--version auto`, merge, then `just release-tag` (Cargo.toml's architect
comment says why).

What it changes, and nothing else:
  - every `tag = "<old>"` on a FastTrackStudios/architect git dependency, in
    every tracked Cargo.toml (the [workspace.dependencies] pins, the
    [patch] table, features/ui's architect-workspace);
  - the old tag where a comment or AGENTS.md mentions Architect on the same
    line ("Pinned at the revision architect v0.12.3 pins");
  - Cargo.lock, by letting cargo resolve the edited manifests
    (`cargo metadata`, which updates only what the edit requires);
  - with --version, `[workspace.package] version`.

Exit status:
  0  the bump is mechanical: only the above changed. The bot may merge it
     once CI is green.
  3  the bump is done, but needs a person: Architect moved a pin this repo
     follows (the editor, dioxus or blitz revision), which the script does
     not move; a major change or a minor jump of more than one (a mistaken
     high tag must not merge itself); or Cargo.lock gained a git source
     repository it did not have. The reasons are printed and, with
     --report FILE, written there.
  1  refused (not a stable tag, a downgrade, split pins, no such tag, or a
     tag whose commit is not on Architect's main: anyone who can push a tag
     can make the tag's own release workflow sign it).

Already on the tag: exit 0 with nothing changed.
The CI bot (.github/workflows/bump-architect.yml) runs exactly this.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.request
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARCHITECT = "FastTrackStudios/architect"
STABLE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
# A git dependency's pin: the repo (host-insensitive) and its rev/tag/branch.
GIT_PIN = re.compile(
    r'git = "https://(?:www\.)?github\.com/([^"]+?)(?:\.git)?"[^}\n]*?\b(?:rev|tag|branch) = "([^"]+)"'
)
ARCHITECT_TAG = re.compile(
    r'(git = "https://(?:www\.)?github\.com/' + ARCHITECT + r'"[^}\n]*?\btag = ")(v[^"]+)(")'
)
# Repos whose pins follow Architect's own (its Cargo.toml says which revision).
FOLLOWED = ("dioxuslabs/dioxus", "fasttrackstudios/blitz", "fasttrackstudios/editor",
            "fasttrackstudios/vendor", "fasttrackstudios/lucide",
            "fasttrackstudios/dioxus-components", "fasttrackstudios/lumen-blocks")
# Text files besides the manifests whose Architect mentions move with the pin.
DOCS = ("AGENTS.md",)


def version(tag: str) -> tuple[int, int, int]:
    m = STABLE.match(tag)
    if not m:
        sys.exit(f"bump-architect: {tag!r} is not a stable release tag (vX.Y.Z)")
    return tuple(int(x) for x in m.groups())  # type: ignore[return-value]


def tracked(*patterns: str) -> list[Path]:
    out = subprocess.run(["git", "ls-files", "-z", "--", *patterns], cwd=ROOT,
                         check=True, capture_output=True, text=True).stdout
    return [ROOT / p for p in out.split("\0") if p]


def pins(text: str) -> dict[str, set[str]]:
    found: dict[str, set[str]] = defaultdict(set)
    for repo, ref in GIT_PIN.findall(text):
        found[repo.lower()].add(ref)
    return found


def fetch_text(url: str) -> str | None:
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            return r.read().decode()
    except Exception:  # noqa: BLE001
        return None


def architect_manifest(tag: str) -> str:
    url = f"https://raw.githubusercontent.com/{ARCHITECT}/refs/tags/{tag}/Cargo.toml"
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            return r.read().decode()
    except Exception as e:  # noqa: BLE001 - any failure means "no such tag"
        sys.exit(f"bump-architect: cannot read {ARCHITECT} {tag}'s Cargo.toml ({e})")



def github(path: str) -> dict:
    req = urllib.request.Request(f"https://api.github.com/{path}",
                                 headers={"Accept": "application/vnd.github+json"})
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def commit_of(repo: str, tag: str) -> str:
    """The commit TAG names (refs/tags/<tag>, peeled): never a branch that
    happens to share a tag's name."""
    out = subprocess.run(["git", "ls-remote", f"https://github.com/{repo}", f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}"],
                         check=True, capture_output=True, text=True).stdout.split("\n")
    shas = {line.split("\t")[1]: line.split("\t")[0] for line in out if "\t" in line}
    sha = shas.get(f"refs/tags/{tag}^{{}}") or shas.get(f"refs/tags/{tag}")
    if not sha:
        sys.exit(f"bump-architect: {repo} has no tag {tag}")
    return sha


def on_main(repo: str, tag: str) -> bool:
    """Is the commit of `tag` (resolved as refs/tags/) on `repo`'s main?"""
    sha = commit_of(repo, tag)
    try:
        return github(f"repos/{repo}/compare/main...{sha}").get("status") in ("behind", "identical")
    except Exception as e:  # noqa: BLE001
        sys.exit(f"bump-architect: cannot tell whether {repo} {tag} ({sha}) is on main ({e})")


def jump(old: str, new: str) -> str | None:
    """Why moving old -> new is not mechanical, if it is not."""
    (omaj, omin, _), (nmaj, nmin, _) = version(old), version(new)
    if nmaj != omaj:
        return f"a major version change ({old} -> {new})"
    if nmin - omin > 1:
        return f"a minor jump of {nmin - omin} ({old} -> {new})"
    return None


def lock_packages(text: str) -> dict[tuple[str, str], str]:
    """(name, version) -> source, for every [[package]] of a Cargo.lock."""
    out = {}
    for block in text.split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', block, re.M)
        ver = re.search(r'^version = "([^"]+)"', block, re.M)
        src = re.search(r'^source = "([^"]+)"', block, re.M)
        if name and ver:
            out[(name.group(1), ver.group(1))] = src.group(1) if src else ""
    return out


def git_repos(packages: dict[tuple[str, str], str]) -> set[str]:
    """The git repositories a lock's packages come from (host-insensitive)."""
    return {re.sub(r"^git\+https://(www\.)?", "", src).split("?")[0].split("#")[0].lower()
            for src in packages.values() if src.startswith("git+")}


def seed_from_upstream(before: dict, upstream_lock: str) -> list[str]:
    """New registry crates the resolve added: take Architect's own versions.

    cargo picks the newest compatible version of a crate it has not locked
    before; Architect's Cargo.lock at the tag says which one Architect
    built and tested. Where they differ, `cargo update --precise` moves the
    new crate to Architect's (best effort: a version cargo cannot take
    there stays, and is reported)."""
    after = lock_packages((ROOT / "Cargo.lock").read_text())
    theirs: dict[str, set[str]] = defaultdict(set)
    for (name, ver), src in lock_packages(upstream_lock).items():
        if src.startswith("registry+"):
            theirs[name].add(ver)
    notes = []
    for (name, ver), src in sorted(after.items()):
        if (name, ver) in before or not src.startswith("registry+") or not theirs.get(name) or ver in theirs[name]:
            continue
        want = max(theirs[name], key=lambda v: [int(x) if x.isdigit() else 0 for x in re.split(r"[.+-]", v)])
        r = subprocess.run(["cargo", "update", "-p", f"{name}@{ver}", "--precise", want], cwd=ROOT,
                           capture_output=True, text=True)
        notes.append(f"{name} {ver} -> {want} (Architect's)" if r.returncode == 0
                     else f"{name} {ver} kept (Architect has {want}; cargo refused --precise)")
    return notes

def token(tag: str) -> re.Pattern[str]:
    # The tag as a whole token: v0.12.3 but not v0.12.30 or v0.12.3.1.
    return re.compile(r"(?<![\w.])" + re.escape(tag) + r"(?![\w])(?!\.\d)")


def rewrite(path: Path, old: str, new: str, word: str) -> bool:
    """`old` -> `new` on the lines of `path` that mention `word` (any case)."""
    text = path.read_text()
    tok = token(old)
    lines = []
    for line in text.splitlines(keepends=True):
        if word in line.lower():
            line = tok.sub(new, line)
        lines.append(line)
    out = "".join(lines)
    if out != text:
        path.write_text(out)
        return True
    return False


def followed_moves(ours: str, arch_old: str, arch_new: str) -> list[str]:
    """Pins this repo shares with Architect that Architect moved between tags."""
    mine, before, after = pins(ours), pins(arch_old), pins(arch_new)
    moves = []
    for repo in FOLLOWED:
        stale = (mine.get(repo, set()) & before.get(repo, set())) - after.get(repo, set())
        if stale:
            now = sorted(after.get(repo, set()) - before.get(repo, set())) or ["(dropped)"]
            moves.append(f"{repo}: architect moved {', '.join(sorted(stale))} -> {', '.join(now)}; "
                         "move this repo's pin with it (and whatever pins the same revision)")
    return moves


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("tag", help="the Architect release tag, vX.Y.Z")
    ap.add_argument("--version", help="also set [workspace.package] version (a keyflow release)")
    ap.add_argument("--report", type=Path, help="write the reasons a person is needed here")
    ap.add_argument("--allow-downgrade", action="store_true")
    ap.add_argument("--no-lock", action="store_true", help="leave Cargo.lock alone (tests)")
    args = ap.parse_args()

    new = args.tag
    manifest = ROOT / "Cargo.toml"
    current = {m.group(2) for p in tracked("Cargo.toml", "*/Cargo.toml")
               for m in ARCHITECT_TAG.finditer(p.read_text())}
    if len(current) != 1:
        sys.exit(f"bump-architect: the Architect pins disagree ({', '.join(sorted(current)) or 'none'}); "
                 "make them one tag by hand first")
    old = current.pop()
    if old == new and not args.version:
        print(f"bump-architect: already on {new}")
        return 0
    if version(new) < version(old) and not args.allow_downgrade:
        sys.exit(f"bump-architect: {new} is older than the current pin {old} (--allow-downgrade)")

    if old != new and not on_main(ARCHITECT, new):
        sys.exit(f"bump-architect: {ARCHITECT} {new} is not on its main: a tag pushed off main is never taken")
    arch_old, arch_new = architect_manifest(old), architect_manifest(new)
    moves = followed_moves(manifest.read_text(), arch_old, arch_new)
    if old != new and (why := jump(old, new)):
        moves.append(why + ": a person checks it is meant")

    changed = []
    if old != new:
        # The pins and the comments alike: every line that names Architect.
        for path in tracked("Cargo.toml", "*/Cargo.toml", *DOCS):
            if rewrite(path, old, new, "architect"):
                changed.append(path)

    if args.version == "auto":
        cur = re.search(r'\[workspace\.package\][^\[]*?\nversion = "(\d+)\.(\d+)\.(\d+)"', manifest.read_text())
        if not cur:
            sys.exit("bump-architect: no [workspace.package] version in Cargo.toml")
        args.version = f"{cur.group(1)}.{cur.group(2)}.{int(cur.group(3)) + 1}"
    if args.version:
        text = manifest.read_text()
        out, n = re.subn(r'(\[workspace\.package\][^\[]*?\nversion = ")[^"]+(")',
                         lambda m: m.group(1) + args.version + m.group(2), text, count=1)
        if n != 1:
            sys.exit("bump-architect: no [workspace.package] version in Cargo.toml")
        if out != text:
            manifest.write_text(out)
            changed.append(manifest)

    if not args.no_lock:
        before = lock_packages((ROOT / "Cargo.lock").read_text())
        # Resolve the edited manifests: cargo rewrites only the entries the
        # edit changes (the architect sources, the members' version).
        subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=ROOT, check=True,
                       stdout=subprocess.DEVNULL)
        upstream_lock = fetch_text(f"https://raw.githubusercontent.com/{ARCHITECT}/refs/tags/{new}/Cargo.lock")
        if upstream_lock:
            for note in seed_from_upstream(before, upstream_lock):
                print(f"  seeded: {note}")
        new_repos = git_repos(lock_packages((ROOT / "Cargo.lock").read_text())) - git_repos(before)
        if new_repos:
            moves.append("Cargo.lock gained git source repositories it did not have: "
                         + ", ".join(sorted(new_repos)) + " (unreviewed code: a person looks first)")
        stale = [line for line in (ROOT / "Cargo.lock").read_text().splitlines()
                 if f"{ARCHITECT}?tag={old}#" in line]
        if old != new and stale:
            sys.exit(f"bump-architect: Cargo.lock still holds architect {old} ({len(stale)} entries)")

    for path in sorted(set(changed)):
        print(f"  {path.relative_to(ROOT)}")
    kind = "minor" if version(new)[:2] != version(old)[:2] else "patch"
    print(f"bump-architect: architect {old} -> {new} ({kind})" + (f", version {args.version}" if args.version else ""))
    if kind == "minor":
        print("bump-architect: a MINOR: Studio needs a keyflow release on it: merge, then `just release-tag`"
              + ("" if args.version else " (after setting the version: --version auto)"))
    if moves:
        msg = "Needs a person: " + "\n".join(["", *[f"- {m}" for m in moves]])
        print(msg, file=sys.stderr)
        if args.report:
            args.report.write_text(msg + "\n")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())

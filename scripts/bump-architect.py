#!/usr/bin/env python3
"""Move every Architect pin in this repo to one new release tag.

    just bump-architect v0.12.5                  # the pins, the comments, Cargo.lock
    just bump-architect v0.12.5 --version 0.5.4  # and the workspace version (a keyflow release)

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
     not move. The reasons are printed and, with --report FILE, written there.
  1  refused (not a stable tag, a downgrade, split pins, no such tag).

Already on the tag: exit 0 with nothing changed.
The CI bot (.github/workflows/bump-architect.yml) runs exactly this.
"""
from __future__ import annotations

import argparse
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


def architect_manifest(tag: str) -> str:
    url = f"https://raw.githubusercontent.com/{ARCHITECT}/refs/tags/{tag}/Cargo.toml"
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            return r.read().decode()
    except Exception as e:  # noqa: BLE001 - any failure means "no such tag"
        sys.exit(f"bump-architect: cannot read {ARCHITECT} {tag}'s Cargo.toml ({e})")


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

    arch_old, arch_new = architect_manifest(old), architect_manifest(new)
    moves = followed_moves(manifest.read_text(), arch_old, arch_new)

    changed = []
    if old != new:
        # The pins and the comments alike: every line that names Architect.
        for path in tracked("Cargo.toml", "*/Cargo.toml", *DOCS):
            if rewrite(path, old, new, "architect"):
                changed.append(path)

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
        # Resolve the edited manifests: cargo rewrites only the entries the
        # edit changes (the architect sources, the members' version).
        subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=ROOT, check=True,
                       stdout=subprocess.DEVNULL)
        stale = [line for line in (ROOT / "Cargo.lock").read_text().splitlines()
                 if f"{ARCHITECT}?tag={old}#" in line]
        if old != new and stale:
            sys.exit(f"bump-architect: Cargo.lock still holds architect {old} ({len(stale)} entries)")

    for path in sorted(set(changed)):
        print(f"  {path.relative_to(ROOT)}")
    print(f"bump-architect: architect {old} -> {new}" + (f", version {args.version}" if args.version else ""))
    if moves:
        msg = "Needs a person: " + "\n".join(["", *[f"- {m}" for m in moves]])
        print(msg, file=sys.stderr)
        if args.report:
            args.report.write_text(msg + "\n")
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())

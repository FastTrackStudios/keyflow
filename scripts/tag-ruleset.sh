#!/usr/bin/env bash
# The v* tag ruleset keyflow lacks (studio has the same one): only
# repository admins and the fts-forge App may create, move or delete a
# release tag. Without it any token with contents:write here can push a
# v* tag, and Studio's bump bot would take a keyflow release from it (its
# `--keyflow auto` also requires the tag's commit to be on main, as a
# second guard). For the OWNER to run once (it needs admin on the repo):
#
#   scripts/tag-ruleset.sh            print the ruleset JSON
#   scripts/tag-ruleset.sh --apply    create it (gh as a repo admin)
set -euo pipefail
FTS_FORGE_APP_ID=5391924   # bypass: `just release-tag` run by an agent
ruleset="$(cat <<JSON
{
  "name": "v* release tags (fts-forge and admins only)",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "rules": [ { "type": "creation" }, { "type": "update" }, { "type": "deletion" } ],
  "bypass_actors": [
    { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" },
    { "actor_id": $FTS_FORGE_APP_ID, "actor_type": "Integration", "bypass_mode": "always" }
  ]
}
JSON
)"
if [ "${1:-}" = --apply ]; then
    gh api -X POST repos/FastTrackStudios/keyflow/rulesets --input - <<<"$ruleset" --jq '"created ruleset \(.id): \(.name)"'
else
    echo "$ruleset"
fi

#!/usr/bin/env bash
# Gate a saved OpenTofu plan before `tofu apply <planfile>`.
#
# Usage (from the root the plan was made in):
#   tofu plan -out=tfplan
#   ../../../check-plan.sh tfplan        # or: deploy/check-plan.sh <planfile> from anywhere
#   tofu apply tfplan
#
# Refuses, always (no override):
#   delete or replace of aws_ebs_volume, aws_eip, aws_route53_zone or aws_s3_bucket.
#   The data volume holds the PLC rotation key and the did:plc -- an identity that cannot be
#   re-minted -- and the EIP is the address the hostname resolves to.
#
# Refuses unless OPENLORE_ALLOW_DELETE=1:
#   any other delete or replace (e.g. replacing the instance, which is cattle but still a
#   stop of the PDS). Set the variable for that one command, after reading the plan.
#
# The argument may also be the JSON from `tofu show -json <planfile>` (a *.json file), which is
# how the gate is tested without credentials.

set -euo pipefail

usage() { echo "usage: $0 <planfile | plan.json>" >&2; exit 2; }
[ $# -eq 1 ] || usage
PLAN="$1"
[ -f "$PLAN" ] || { echo "check-plan: no such file: $PLAN" >&2; exit 2; }
command -v jq >/dev/null || { echo "check-plan: jq is required" >&2; exit 2; }

if [[ "$PLAN" == *.json ]]; then
  JSON=$(cat "$PLAN")
else
  JSON=$(tofu show -json "$PLAN")
fi

PROTECTED='["aws_ebs_volume","aws_eip","aws_route53_zone","aws_s3_bucket"]'

# "address  actions" for every change whose actions include delete (a replace is
# ["delete","create"] or ["create","delete"], so it is caught by the same test).
deletes() {
  jq -r --argjson protected "$PROTECTED" --arg mode "$1" '
    [.resource_changes // [] | .[]
      | select(.change.actions | index("delete"))
      | select(.mode == "managed")
      | . as $rc
      | select((($protected | index($rc.type)) != null) == ($mode == "protected"))
      | "\(.address)  [\(.change.actions | join(","))]"]
    | .[]' <<<"$JSON"
}

protected_hits=$(deletes protected)
other_hits=$(deletes other)
status=0

if [ -n "$protected_hits" ]; then
  echo "REFUSED: this plan deletes or replaces a protected resource:" >&2
  while IFS= read -r line; do echo "    $line" >&2; done <<<"$protected_hits"
  echo "  The volume holds the PDS identity; the EIP, zone and buckets are not cattle." >&2
  echo "  There is no override. If this is truly intended, it is a reviewed commit and a" >&2
  echo "  human-run apply without this gate." >&2
  status=1
fi

if [ -n "$other_hits" ]; then
  if [ "${OPENLORE_ALLOW_DELETE:-}" = "1" ]; then
    echo "allowed by OPENLORE_ALLOW_DELETE=1 (delete/replace):"
    while IFS= read -r line; do echo "    $line"; done <<<"$other_hits"
  else
    echo "REFUSED: this plan deletes or replaces resources:" >&2
    while IFS= read -r line; do echo "    $line" >&2; done <<<"$other_hits"
    echo "  Read the plan. If the delete is intended, re-run with OPENLORE_ALLOW_DELETE=1." >&2
    status=1
  fi
fi

if [ "$status" -eq 0 ]; then
  echo "check-plan: no protected resource is deleted or replaced -- OK to apply $PLAN"
fi
exit "$status"

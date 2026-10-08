#!/usr/bin/env bash
# render-secrets.sh -- runs ON THE HOST as root (instance role), never in the container.
# Installed to /pds/app/bin/render-secrets.sh; invoked by deploy.sh over SSM Run Command.
#
# Reads every SSM SecureString under /openlore/prod/review-app/ and writes one file per
# parameter (named by the parameter's last path segment) into /pds/app/secrets, 0400 and owned
# by the container uid (distroless nonroot, 65532). The container mounts that directory
# read-only at /run/secrets. Values never reach stdout or stderr, so SSM command output stays
# clean. See devops/infrastructure-integration.md §5.
#
# The container bind-mounts the DIRECTORY and pins its inode, so the directory itself is never
# renamed, removed or recreated (D3). Each value goes to a dot-prefixed temp file in the same
# directory (same filesystem, so the rename is atomic) and is renamed over its final name only
# after every required parameter has been read. A render missing a required parameter leaves
# the directory untouched. Files of parameters that no longer exist are removed afterwards.
# Parameter names may not start with a dot, so a temp name never equals a parameter's name.
set -euo pipefail
set +x

SSM_PATH="${REVIEW_APP_SSM_PATH:-/openlore/prod/review-app/}"
SECRETS_DIR="${REVIEW_APP_SECRETS_DIR:-/pds/app/secrets}"
APP_UID="${REVIEW_APP_UID:-65532}"
REGION="${REVIEW_APP_REGION:-us-east-1}"
REQUIRED=(client-jwk data-key github-token log-salt)

umask 077
if [ ! -d "$SECRETS_DIR" ]; then
  mkdir -p "$SECRETS_DIR"
  chown "$APP_UID:$APP_UID" "$SECRETS_DIR"
  chmod 0700 "$SECRETS_DIR"
fi

staged="" # space-separated rendered names; each has a temp file "$SECRETS_DIR/.<name>.new"
temp_of() { printf '%s/.%s.new' "$SECRETS_DIR" "$1"; }
cleanup() {
  local file
  for file in $staged; do
    rm -f "$(temp_of "$file")"
  done
}
trap cleanup EXIT

names=$(aws ssm get-parameters-by-path --region "$REGION" --path "$SSM_PATH" \
  --query 'Parameters[].Name' --output text)

count=0
for name in $names; do
  file="${name##*/}"
  case "$file" in
    "" | .* | *[!A-Za-z0-9._-]*)
      echo "render-secrets: skipping parameter with an unsafe name" >&2
      continue
      ;;
  esac
  staged="$staged $file"
  aws ssm get-parameter --region "$REGION" --name "$name" --with-decryption \
    --query 'Parameter.Value' --output text >"$(temp_of "$file")"
  count=$((count + 1))
done

for want in "${REQUIRED[@]}"; do
  if [ ! -s "$(temp_of "$want")" ]; then
    echo "render-secrets: required parameter ${SSM_PATH}${want} is missing or empty" >&2
    exit 1
  fi
done

for file in $staged; do
  chown "$APP_UID:$APP_UID" "$(temp_of "$file")"
  chmod 0400 "$(temp_of "$file")"
done

# rename(2): an atomic FILE replace; the directory inode never changes. The running container
# sees the new files at once but reads secrets only at startup, so rotation stays restart-based.
for file in $staged; do
  mv -f "$(temp_of "$file")" "$SECRETS_DIR/$file"
done

# Remove files of parameters that no longer exist (never the directory).
while IFS= read -r -d '' path; do
  case " $staged " in
    *" ${path##*/} "*) ;;
    *) rm -f "$path" ;;
  esac
done < <(find "$SECRETS_DIR" -mindepth 1 -maxdepth 1 -type f -print0)

echo "render-secrets: rendered $count parameter(s) into $SECRETS_DIR"

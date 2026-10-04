#!/usr/bin/env bash
# render-secrets.sh -- runs ON THE HOST as root (instance role), never in the container.
# Installed to /pds/app/bin/render-secrets.sh; invoked by deploy.sh over SSM Run Command.
#
# Reads every SSM SecureString under /openlore/prod/review-app/ and writes one file per
# parameter (named by the parameter's last path segment) into /pds/app/secrets, 0400 and owned
# by the container uid (distroless nonroot, 65532). The container mounts that directory
# read-only at /run/secrets. Values never reach stdout or stderr, so SSM command output stays
# clean. See devops/infrastructure-integration.md §5.
set -euo pipefail
set +x

SSM_PATH="${REVIEW_APP_SSM_PATH:-/openlore/prod/review-app/}"
SECRETS_DIR="${REVIEW_APP_SECRETS_DIR:-/pds/app/secrets}"
APP_UID="${REVIEW_APP_UID:-65532}"
REGION="${REVIEW_APP_REGION:-us-east-1}"
REQUIRED=(client-jwk data-key github-token log-salt)

umask 077
parent=$(dirname "$SECRETS_DIR")
staging=$(mktemp -d "$parent/.secrets.new.XXXXXX")
cleanup() { rm -rf "$staging" "$parent/.secrets.old"; }
trap cleanup EXIT

names=$(aws ssm get-parameters-by-path --region "$REGION" --path "$SSM_PATH" \
  --query 'Parameters[].Name' --output text)

count=0
for name in $names; do
  file="${name##*/}"
  case "$file" in
    "" | *[!A-Za-z0-9._-]*)
      echo "render-secrets: skipping parameter with an unsafe name" >&2
      continue
      ;;
  esac
  aws ssm get-parameter --region "$REGION" --name "$name" --with-decryption \
    --query 'Parameter.Value' --output text >"$staging/$file"
  count=$((count + 1))
done

for want in "${REQUIRED[@]}"; do
  if [ ! -s "$staging/$want" ]; then
    echo "render-secrets: required parameter ${SSM_PATH}${want} is missing or empty" >&2
    exit 1
  fi
done

chown -R "$APP_UID:$APP_UID" "$staging"
find "$staging" -type f -exec chmod 0400 {} +
chmod 0700 "$staging"

# Swap the directory in. The running container keeps the old inode until it restarts, which is
# when it reads secrets anyway.
if [ -e "$SECRETS_DIR" ]; then
  mv "$SECRETS_DIR" "$parent/.secrets.old"
fi
mv "$staging" "$SECRETS_DIR"

echo "render-secrets: rendered $count parameter(s) into $SECRETS_DIR"

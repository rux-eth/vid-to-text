#!/usr/bin/env bash
# Deploy version-controlled profiles to the server.
#
# The repo is the source of truth. Phase 1 of PR-020 found all 11 profiles
# existed ONLY on the desktop, untracked -- a config with no history and no
# review. This closes that.
#
#   ./config/deploy-profiles.sh            dry run
#   ./config/deploy-profiles.sh --apply
set -uo pipefail
HOST="${VTT_HOST:-rux@100.90.42.41}"
REMOTE="${VTT_REMOTE_CFG:-/home/rux/.vid-to-text/config/profiles}"
SRC="$(cd "$(dirname "$0")/profiles" && pwd)"
APPLY=0; [[ "${1:-}" == "--apply" ]] && APPLY=1

echo "source : $SRC"
echo "host   : $HOST:$REMOTE"
echo "mode   : $([[ $APPLY == 1 ]] && echo APPLY || echo 'DRY RUN (pass --apply)')"
echo
for f in "$SRC"/*.toml; do
  b=$(basename "$f")
  remote_sum=$(ssh -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "sha256sum $REMOTE/$b 2>/dev/null | cut -d' ' -f1" 2>/dev/null)
  local_sum=$(shasum -a 256 "$f" | cut -d' ' -f1)
  if [[ "$remote_sum" == "$local_sum" ]]; then
    echo "  = $b (in sync)"
  elif [[ -z "$remote_sum" ]]; then
    echo "  + $b (absent on server)"
  else
    echo "  ! $b (DIFFERS - server copy will be overwritten)"
  fi
done
echo
(( APPLY == 0 )) && { echo "Nothing copied. Re-run with --apply."; exit 0; }
ssh -o BatchMode=yes "$HOST" "mkdir -p $REMOTE" || exit 1
scp -o BatchMode=yes -q "$SRC"/*.toml "$HOST:$REMOTE/" || exit 1
echo "deployed. verifying:"
for f in "$SRC"/*.toml; do
  b=$(basename "$f")
  r=$(ssh -o BatchMode=yes "$HOST" "sha256sum $REMOTE/$b | cut -d' ' -f1")
  l=$(shasum -a 256 "$f" | cut -d' ' -f1)
  [[ "$r" == "$l" ]] && echo "  ok $b" || echo "  MISMATCH $b"
done

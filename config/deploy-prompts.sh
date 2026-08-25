#!/usr/bin/env bash
# Deploy version-controlled prompts to the server. (PR-026)
#
# The repo is the source of truth. Before this existed, NOTHING deployed prompts/:
# deploy-profiles.sh copies *.toml only, and ~/vid-to-text on the desktop is an
# unversioned file copy (verified 2026-08-25: `fatal: not a git repository`), so a
# prompt reached the GPU only by hand, with nothing verifying that what ran was what
# was intended. Every timeline now records the prompt's SHA-256 (CaptureInfo), and
# the sums printed here are the same value -- so a timeline can be checked against
# what is deployed.
#
# Lives beside deploy-profiles.sh so the two deploy paths are found together, even
# though prompts are not config.
#
# The destination is the server's WORKING DIRECTORY, not a config dir:
# ollama.prompt_template_path defaults to the relative "prompts/vision.txt", and the
# systemd unit sets WorkingDirectory=/home/rux/vid-to-text.
#
#   ./config/deploy-prompts.sh            dry run
#   ./config/deploy-prompts.sh --apply
set -uo pipefail
HOST="${VTT_HOST:-rux@100.90.42.41}"
REMOTE="${VTT_REMOTE_PROMPTS:-/home/rux/vid-to-text/prompts}"
SRC="$(cd "$(dirname "$0")/../prompts" && pwd)"
APPLY=0; [[ "${1:-}" == "--apply" ]] && APPLY=1

echo "source : $SRC"
echo "host   : $HOST:$REMOTE"
echo "mode   : $([[ $APPLY == 1 ]] && echo APPLY || echo 'DRY RUN (pass --apply)')"
echo
changed=0
for f in "$SRC"/*.txt; do
  b=$(basename "$f")
  remote_sum=$(ssh -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "sha256sum $REMOTE/$b 2>/dev/null | cut -d' ' -f1" 2>/dev/null)
  local_sum=$(shasum -a 256 "$f" | cut -d' ' -f1)
  if [[ "$remote_sum" == "$local_sum" ]]; then
    echo "  = $b (in sync)  ${local_sum:0:16}"
  elif [[ -z "$remote_sum" ]]; then
    echo "  + $b (absent on server)  ${local_sum:0:16}"; changed=1
  else
    echo "  ! $b (DIFFERS - server copy will be overwritten)"
    echo "      local  ${local_sum:0:16}"
    echo "      remote ${remote_sum:0:16}"; changed=1
  fi
done
echo
(( APPLY == 0 )) && { echo "Nothing copied. Re-run with --apply."; exit 0; }
(( changed == 0 )) && { echo "Already in sync; nothing to do."; exit 0; }

ssh -o BatchMode=yes "$HOST" "mkdir -p $REMOTE" || exit 1
scp -o BatchMode=yes -q "$SRC"/*.txt "$HOST:$REMOTE/" || exit 1

# Verify AFTER copying. A silent partial copy would leave the GPU running a prompt
# no timeline records, which is the failure this script exists to prevent.
echo "deployed. verifying:"
bad=0
for f in "$SRC"/*.txt; do
  b=$(basename "$f")
  r=$(ssh -o BatchMode=yes "$HOST" "sha256sum $REMOTE/$b | cut -d' ' -f1")
  l=$(shasum -a 256 "$f" | cut -d' ' -f1)
  if [[ "$r" == "$l" ]]; then echo "  ok $b  ${l:0:16}"; else echo "  MISMATCH $b"; bad=1; fi
done
(( bad == 1 )) && { echo; echo "VERIFY FAILED - do not run a capture against this server."; exit 1; }
echo
echo "The server loads the prompt in OllamaClient::new, which runs per job (pipeline.rs:94), so a"
echo "server picks up the new prompt on its next job. No restart needed."

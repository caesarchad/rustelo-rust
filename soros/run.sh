#!/usr/bin/env bash
#
# Run a minimal Bitconch cluster.  Ctrl-C to exit.
#
# Before running this script ensure standard Bitconch programs are available
# in the PATH, or that `cargo build --all` ran successfully
#
set -e

# Prefer possible `cargo build --all` binaries over PATH binaries
PATH=$PWD/target/debug:$PATH

ok=true
for program in bitconch-{genesis,keygen,fullnode{,-config}}; do
  $program -V || ok=false
done
$ok || {
  echo
  echo "Unable to locate required programs.  Try building them first with:"
  echo
  echo "  $ cargo build --all"
  echo
  exit 1
}

blockstreamSocket=/tmp/bitconch-blockstream.sock # Default to location used by the block explorer
while [[ -n $1 ]]; do
  if [[ $1 = --blockstream ]]; then
    blockstreamSocket=$2
    shift 2
  else
    echo "Unknown argument: $1"
    exit 1
  fi
done

export RUST_LOG=${RUST_LOG:-bitconch=info} # if RUST_LOG is unset, default to info
export RUST_BACKTRACE=1
dataDir=$PWD/target/"$(basename "$0" .sh)"

set -x
bitconch-keygen -o "$dataDir"/config/leader-keypair.json
bitconch-keygen -o "$dataDir"/config/drone-keypair.json

bitconch-fullnode-config \
  --keypair="$dataDir"/config/leader-keypair.json -l > "$dataDir"/config/leader-config.json
bitconch-genesis \
  --num_tokens 1000000000 \
  --mint "$dataDir"/config/drone-keypair.json \
  --bootstrap-leader-keypair "$dataDir"/config/leader-keypair.json \
  --ledger "$dataDir"/ledger

bitconch-drone --keypair "$dataDir"/config/drone-keypair.json &
drone=$!

args=(
  --identity "$dataDir"/config/leader-config.json
  --ledger "$dataDir"/ledger/
  --rpc-port 8899
)
if [[ -n $blockstreamSocket ]]; then
  args+=(--blockstream "$blockstreamSocket")
fi
bitconch-fullnode "${args[@]}" &
fullnode=$!

abort() {
  kill "$drone" "$fullnode"
}

trap abort SIGINT SIGTERM
wait "$fullnode"
kill "$drone" "$fullnode"

#!/usr/bin/env bash
set -e
#
# This script is to be run on the bootstrap full node
#

cd "$(dirname "$0")"/../..

updateDownloadUrl=$1

[[ -r deployConfig ]] || {
  echo deployConfig missing
  exit 1
}
# shellcheck source=/dev/null # deployConfig is written by remote-node.sh
source deployConfig

missing() {
  echo "Error: $1 not specified"
  exit 1
}

[[ -n $updateDownloadUrl ]] || missing updateDownloadUrl

RUST_LOG="$2"
export RUST_LOG=${RUST_LOG:-soros=info} # if RUST_LOG is unset, default to info

source net/common.sh
loadConfigFile

PATH="$HOME"/.cargo/bin:"$PATH"

set -x
soros-wallet airdrop 42
soros-install deploy "$updateDownloadUrl" update_manifest_keypair.json \
#  --url http://localhost:8899
   --url http://localhost:10099

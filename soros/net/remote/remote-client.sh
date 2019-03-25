#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"/../..

echo "$(date) | $0 $*" > client.log

deployMethod="$1"
entrypointIp="$2"
RUST_LOG="$3"
export RUST_LOG=${RUST_LOG:-bitconch=info} # if RUST_LOG is unset, default to info

missing() {
  echo "Error: $1 not specified"
  exit 1
}

[[ -n $deployMethod ]] || missing deployMethod
[[ -n $entrypointIp ]] || missing entrypointIp

source net/common.sh
loadConfigFile

threadCount=$(nproc)
if [[ $threadCount -gt 4 ]]; then
  threadCount=4
fi

case $deployMethod in
snap)
  net/scripts/rsync-retry.sh -vPrc "$entrypointIp:~/bitconch/bitconch.snap" .
  sudo snap install bitconch.snap --devmode --dangerous

  bitconch_bench_tps=/snap/bin/bitconch.bench-tps
  ;;
local|tar)
  PATH="$HOME"/.cargo/bin:"$PATH"
  export USE_INSTALL=1

  ./fetch-perf-libs.sh
  # shellcheck source=/dev/null
  source ./target/perf-libs/env.sh

  net/scripts/rsync-retry.sh -vPrc "$entrypointIp:~/.cargo/bin/bitconch*" ~/.cargo/bin/
  bitconch_bench_tps=bitconch-bench-tps
  ;;
*)
  echo "Unknown deployment method: $deployMethod"
  exit 1
esac

(
  sudo scripts/oom-monitor.sh
) > oom-monitor.log 2>&1 &
scripts/net-stats.sh  > net-stats.log 2>&1 &

! tmux list-sessions || tmux kill-session

clientCommand="\
  $bitconch_bench_tps \
    --network $entrypointIp:8001 \
    --drone $entrypointIp:9900 \
    --duration 7500 \
    --sustained \
    --threads $threadCount \
"

tmux new -s bitconch-bench-tps -d "
  while true; do
    echo === Client start: \$(date) | tee -a client.log
    $metricsWriteDatapoint 'testnet-deploy client-begin=1'
    echo '$ $clientCommand' | tee -a client.log
    $clientCommand >> client.log 2>&1
    $metricsWriteDatapoint 'testnet-deploy client-complete=1'
  done
"
sleep 1
tmux capture-pane -t bitconch-bench-tps -p -S -100
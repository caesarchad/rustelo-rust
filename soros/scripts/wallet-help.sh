#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"/..

cargo build
export PATH=$PWD/target/debug:$PATH

echo "\`\`\`manpage"
bitconch-wallet --help
echo "\`\`\`"
echo ""

commands=(address airdrop balance cancel confirm deploy get-transaction-count pay send-signature send-timestamp)

for x in "${commands[@]}"; do
    echo "\`\`\`manpage"
    bitconch-wallet "${x}" --help
    echo "\`\`\`"
    echo ""
done

#!/usr/bin/env bash

set -euo pipefail

# VID:PID of our example device in runtime mode
runtime_id='1209:0001'

if (( $# > 0 )) && [[ "$1" =~ -h|--help ]]; then
  echo "Usage: $(basename $0)"
  echo "Request USB DFU detach to jump from runtime mode to DFU mode."
  exit 0
fi

dfu-util --detach --device "$runtime_id"

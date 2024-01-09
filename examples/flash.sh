#!/usr/bin/env bash

set -euo pipefail

if (( $# > 0 )) && [[ "$1" =~ -h|--help ]]; then
  echo "Usage: $(basename $0)"
  echo "Compile firmware and flash it over USB-DFU using the embedded STM32 bootloader"
  exit 0
fi

# VID:PID of the embedded STM32 DFU bootloader
stm32dfu_id='0483:df11'

name="usbd-dfu-rt-example"
binary="target/$name.bin"

cargo build --release
cargo objcopy --release --bin "$name" -- -O binary "$binary"

dfu-util \
    --device "$stm32dfu_id" \
    --alt 0 \
    --dfuse-address "0x08000000:leave" \
    --download "$binary" \
    --reset

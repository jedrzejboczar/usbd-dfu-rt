# USB DFU runtime class example

This is an example of implementing USB DFU runtime class on STM32F072 MCU,
making use of the fact that this MCU has a builtin DFU-capable bootloader.

For other MCUs it would be necessary to write a custom DFU-capable bootloader
and adjust reboot code accordingly.

## Hardware

This example is not written for any particular board, it should just have the following:

* STM32F072 MCU
* USB connected to MCU on pins (PA11, PA12)
* MCU is powered from USB (some 3.3V voltage regulator)
* Some button connected to MCU's RESET pin
* Some button connected to MCU's BOOT pin

## How to run

* Install [dfu-util](https://dfu-util.sourceforge.net/)
* Connect the board via USB
* Use the BOOT button together with RESET button to force the MCU to the STM32 embedded bootloader
* Use `./flash.sh` to build this example and load it via the DFU bootloader
* (optional) If the MCU does not enumerate via USB, disconnect and connect it again.
* Now the device with VID:PID `1209:0001` should be visible (e.g. using `lsusb`)
* Use `./detach.sh` to perform DFU detach request on the device
* The reboot logic in code will now do the following:

  * call `DfuRuntimeOps::detach`  -> `reboot` (writes the `MAGIC` value)
  * MCU resets
  * `maybe_jump_bootloader` is called before `main`
  * software reset is detected and `MAGIC` value is found in memory
  * `MAGIC` value is reset back to `0`
  * `cortex_m::asm::bootload` is called to jump to embedded STM32 bootloader

* The device should enumerate as `STM32  BOOTLOADER`

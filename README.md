# usbd-dfu-rt [![crates.io](https://img.shields.io/crates/v/usbd-dfu-rt.svg)](https://crates.io/crates/usbd-dfu-rt) [![docs.rs](https://docs.rs/usbd-dfu-rt/badge.svg)](https://docs.rs/usbd-dfu-rt)

This is a Rust crate that implements the USB DFU run-time class for use with the
[usb-device](https://crates.io/crates/usb-device) crate.

# DFU run-time class

DFU stands for Device Firmware Upgrade. DFU defines two USB classes:
* DFU mode: used to transfer new firmware and flash the device
* Run-time: advertises DFU capability and can be used to change device mode to DFU upgrade

This crate implements DFU run-time class according to the USB DFU class specification version
1.1a. It implements _only_ the run-time class, which means that it only implements DFU_DETACH
request. This request means that the device should switch to DFU mode in preparation for
firmware upgrade. This usually means rebooting to a DFU-capable bootloader which then handles
the upgrade.

To use this class user must provide a callback that will perform the transition to DFU mode,
which is highly device-specific.

# Example

Some MCUs may come with an embedded DFU bootloader firmware. This may be used to implement full
firmware update via USB with minimal effort - we only need to implement DFU run-time class, and
DFU mode is implemented in the embedded bootloader. This is e.g. the case for the STM32F072
MCU.

On STM32F072, before jumping to the embedded bootloader, we should disable all peripherals,
setting them to the reset state. This might be problematic to do in our application, but it is
possible to just perform a CPU reset, which will also reset the peripherals and then jump to
the embedded bootloader. For this to work, we need a way for the firmware to detect that a
reset occured because we wanted to jump to DFU bootloader. This can be done by storing a magic
value in the memory and checking it just after reset.

The following code could be used to implement the logic described above. `enter` will be called
on a DFU_DETACH request, setting the magic value and resetting the MCU. The `jump_bootloader`
routine will be executed before any code that initializes RAM (due to the
`#[cortex_m_rt::pre_init]` attribute), so the magic value will still be storing the value
written before reset. It is then checked to see if we should perform a jump to the embedded
bootloader.

```rust
use core::mem::MaybeUninit;
use cortex_m_rt;
use usbd_dfu_rt::DfuRuntimeOps;

const MAGIC_JUMP_BOOTLOADER: u32 = 0xdeadbeef;
const SYSTEM_MEMORY_BASE: u32 = 0x1fffc800;

#[link_section = ".uninit.MAGIC"]
static mut MAGIC: MaybeUninit<u32> = MaybeUninit::uninit();

#[cortex_m_rt::pre_init]
unsafe fn jump_bootloader() {
    if MAGIC.assume_init() == MAGIC_JUMP_BOOTLOADER {
        // reset the magic value not to jump again
        MAGIC.as_mut_ptr().write(0);
        // jump to bootloader located in System Memory
        cortex_m::asm::bootload(SYSTEM_MEMORY_BASE as *const u32);
    }
}

pub struct DFUBootloader;

impl DfuRuntimeOps for DFUBootloader {
    fn enter(&mut self) {
        unsafe { MAGIC.as_mut_ptr().write(MAGIC_JUMP_BOOTLOADER); }
        cortex_m::peripheral::SCB::sys_reset();
    }
}
```

# TODO

Missing functionality:

[ ] Handle timeouts
[ ] Make the descriptors configurable

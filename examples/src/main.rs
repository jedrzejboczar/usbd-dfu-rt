#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use cortex_m::peripheral::{SCB, syst::SystClkSource};
use panic_halt as _;
use cortex_m_rt::entry;
use stm32f0xx_hal as hal;
use hal::{prelude::*, pac};
use usb_device::{device::{UsbVidPid, UsbDeviceBuilder}, prelude::StringDescriptors};
use usbd_dfu_rt::{DfuRuntimeClass, DfuRuntimeOps};

// Location of the embedded bootloader in stm32f072
const SYSTEM_MEMORY_BASE: u32 = 0x1fffc800;

// This value must be found in memory after reset to jump to bootloader
const MAGIC_JUMP_BOOTLOADER: u32 = 0xdeadbeef;

// Memory reserved for the value that will be read before main.
// Stored in uninit section so that runtime will not initialize this value.
#[link_section = ".uninit.MAGIC"]
static mut MAGIC: MaybeUninit<u32> = MaybeUninit::uninit();

// Runs before main and jumps to embedded bootloader if the conditions are met
#[cortex_m_rt::pre_init]
unsafe fn maybe_jump_bootloader() {
    // Verify that this was a software reset
    let software_reset = (*pac::RCC::ptr()).csr.read().sftrstf().bit_is_set();

    if software_reset && MAGIC.assume_init() == MAGIC_JUMP_BOOTLOADER {
        // reset the magic value not to jump again
        MAGIC.as_mut_ptr().write(0);
        // jump to bootloader located in System Memory
        cortex_m::asm::bootload(SYSTEM_MEMORY_BASE as *const u32);
    }
}

// Perform system reboot (to our code) but optionally set MAGIC value for pre_init to read
pub fn reboot(to_bootloader: bool, usb_bus: Option<&hal::usb::UsbBusType>) -> ! {
    if to_bootloader {
        // SAFETY: we're writing to memory that is reserved for that purpose
        unsafe {
            MAGIC.as_mut_ptr().write(MAGIC_JUMP_BOOTLOADER);
        }
    }
    if let Some(bus) = usb_bus {
        // Sometimes host fails to reenumerate our device when jumping to bootloader,
        // so we force reenumeration and only after that we do reset.
        bus.force_reenumeration(|| SCB::sys_reset());
        // not going any further, but not using if-else to satisfy return type
    }
    SCB::sys_reset()
}


// Minimal implementation with no support for timeout
pub struct DfuBootloader;

impl DfuRuntimeOps for DfuBootloader {
    fn detach(&mut self) {
        // I suspect this works without force_reenumeration because we actually reset
        // the system twice: once on sys_reset, then in jump_bootloader, but not sure.
        reboot(true, None)
    }
}

#[entry]
fn main() -> ! {
    let mut p = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut rcc = p.RCC
        .configure()
        .hsi48()
        .sysclk(48.mhz())
        .pclk(24.mhz())
        .enable_crs(p.CRS) // synchronization to USB SOF
        .freeze(&mut p.FLASH);

    // Configure systick to wrap around every millisecond
    let mut systick = cp.SYST;
    systick.set_clock_source(SystClkSource::Core);
    systick.set_reload(48_000 - 1);
    systick.enable_counter();

    let gpioa = p.GPIOA.split(&mut rcc);

    let usb = hal::usb::Peripheral {
        usb: p.USB,
        pin_dp: gpioa.pa12,
        pin_dm: gpioa.pa11
    };
    let usb_bus = hal::usb::UsbBus::new(usb);

    let mut dfu = DfuRuntimeClass::new(&usb_bus, DfuBootloader);

    // https://pid.codes
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0x0001))
        .strings(&[
            StringDescriptors::default()
                .manufacturer("usb-dfu-rt example")
                .product("usb-dfu-rt example")
                .serial_number(env!("CARGO_PKG_VERSION"))
        ])
        .unwrap()
        .build();

    loop {
        // busy wait until the timer wraps around
        while !systick.has_wrapped() {}

        if usb_dev.poll(&mut [&mut dfu]) {
            // noop
        }

        // every 1 ms
        dfu.tick(1);
    }
}

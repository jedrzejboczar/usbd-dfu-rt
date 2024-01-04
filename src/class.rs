use usb_device::class_prelude::*;
use usb_device::Result;

const USB_CLASS_APPLICATION_SPECIFIC: u8 = 0xfe;
const DFU_SUBCLASS_FIRMWARE_UPGRADE: u8 = 0x01;
const DFU_PROTOCOL_RUNTIME: u8 = 0x01;
const DFU_TYPE_FUNCTIONAL: u8 = 0x21;
const DFU_REQ_DETACH: u8 = 0;
const DFU_REQ_GETSTATUS: u8 = 0x03;

#[repr(u8)]
#[derive(Clone, Copy)]
enum DfuState {
    /// Device is running its normal application.
    AppIdle = 0,
    /// Device is running its normal application, has received the DFU_DETACH request, and is waiting for a USB reset.
    AppDetach = 1,
}

#[repr(u8)]
#[derive(Clone, Copy)]
enum DfuStatusCode {
    /// No error condition is present.
    OK = 0x00,
}

/// Implementation of DFU runtime class.
///
/// This class provides thin framework for implementing DFU runtime functionality.
/// When DFU_DETACH request is received, it will be accepted and [`DfuRuntimeOps::detach`]
/// will be called (unless [`DfuRuntimeOps::allow`] returned `false` which rejects the request).
pub struct DfuRuntimeClass<T: DfuRuntimeOps> {
    ops: T,
    iface: InterfaceNumber,
    timeout: Option<u16>,
    state: DfuState,
}

/// Trait that defines device-specific operations for [`DfuRuntimeClass`].
pub trait DfuRuntimeOps {
    /// Switch to DFU mode
    ///
    /// Handler that should reconfigure device to DFU mode. User application should perform
    /// any necessary system cleanup and switch to DFU mode, which most often means that the
    /// application should jump to DFU-capable bootloader.
    ///
    /// # Note
    ///
    /// When [`WILL_DETACH`] is set to `false`, this handler will be called after USB reset
    /// is detected, unless timeout occurs. It is usually simpler to use this mode.
    ///
    /// When [`WILL_DETACH`] is set to `true`, it will **not** be called immediately (because
    /// the detach request needs to be accepted). Instead, the class will wait for the `timeout`
    /// value as returned from [`DfuRuntimeOps::allow`] and when it reaches 0 (in
    /// [`DfuRuntimeClass::tick`]) this method will be called.
    fn detach(&mut self);

    /// Determines whether DFU_DETACH request should be accepted
    ///
    /// This method receives the `wDetachTimeout` value from detach request. Default
    /// implementation accepts all requests using unmodified timeout value.
    ///
    /// This method can be used to reject DFU_DETACH requests (by returning `None`) unless
    /// certain condition is met, e.g. to prevent unauthorized firmware upgrades.
    ///
    /// One could use this method to immediately start some system cleanup jobs, instead
    /// of waiting for call to `DfuRuntimeOps::detach`.
    fn allow(&mut self, timeout: u16) -> Option<u16> {
        Some(timeout)
    }

    /// Device will perform detach-attach sequence on DFU_DETACH, host must not issue USB reset
    ///
    /// This is especially useful if the firmware jumps to bootloader by performing system reset,
    /// so there is no need for host to issue USB reset.
    ///
    /// If this is set to `false` then the device should start a timer counting the amount of
    /// milliseconds in `wDetachTimeout` of DFU_DETACH request. It shall enable DFU mode if USB
    /// reset is detected within this timeout.
    const WILL_DETACH: bool = true;

    /// Bootloader is able to communicate via USB during Manifestation phase
    const MANIFESTATION_TOLERANT: bool = false;

    /// Bootloader can download firmware to device
    const CAN_DNLOAD: bool = true;

    /// Bootloader can read device firmware and upload it to host
    const CAN_UPLOAD: bool = true;

    /// Max time for which the device will wait for USB reset after DFU_DETACH
    ///
    /// The actual time specified in DFU_DETACH `wDetachTimeout` can be lower than this value.
    /// When [`WILL_DETACH`] is set to `true` then device should not wait for USB reset anyway.
    const DETACH_TIMEOUT_MS: u16 = 255;

    /// Bootloader maximum transfer size in bytes per control-write transaction
    const MAX_TRANSFER_SIZE: u16 = 2048;  // Max value for STM32 DFU bootloader
}

impl<T: DfuRuntimeOps> DfuRuntimeClass<T> {
    /// Crate new DFU run-time class with the given device-specific implementations.
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>, ops: T) -> Self {
        Self {
            ops,
            iface: alloc.interface(),
            timeout: None,
            state: DfuState::AppIdle,
        }
    }

    /// Advance time
    ///
    /// Should be called regularly, passing the time in milliseconds that has elapsed since the
    /// previous call to this function.
    pub fn tick(&mut self, elapsed_time_ms: u16) {
        if let Some(timeout) = self.timeout {
            let new = timeout.saturating_sub(elapsed_time_ms);
            if new == 0 {
                self.timeout = None;
                if T::WILL_DETACH {
                    self.ops.detach();
                }
            } else {
                self.timeout = Some(new);
            }
        }
    }

    /// Get reference to [`DfuRuntimeOps`]
    pub fn ops(&self) -> &T {
        &self.ops
    }

    /// Get mutable reference to [`DfuRuntimeOps`]
    pub fn ops_mut(&mut self) -> &mut T {
        &mut self.ops
    }

    /// Get class interface number
    pub fn interface(&self) -> InterfaceNumber {
        self.iface
    }

    const fn dfu_bm_attributes() -> u8 {
        (T::WILL_DETACH as u8) << 3
        | (T::MANIFESTATION_TOLERANT as u8) << 2
        | (T::CAN_DNLOAD as u8) << 1
        | T::CAN_DNLOAD as u8
    }
}

impl<T: DfuRuntimeOps, B: UsbBus> UsbClass<B> for DfuRuntimeClass<T> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        // NOTE: it seems that this is necessary even though we have 1 interface,
        // without IAD dfu-util fails to detach the device
        writer.iad(
            self.iface,
            1,
            USB_CLASS_APPLICATION_SPECIFIC,
            DFU_SUBCLASS_FIRMWARE_UPGRADE,
            DFU_PROTOCOL_RUNTIME,
            None)?;

        writer.interface(
            self.iface,
            USB_CLASS_APPLICATION_SPECIFIC,
            DFU_SUBCLASS_FIRMWARE_UPGRADE,
            DFU_PROTOCOL_RUNTIME)?;

        // Run-Time DFU Functional Descriptor
        let detach_timeout: u16 = T::DETACH_TIMEOUT_MS;
        let transfer_size: u16 = T::MAX_TRANSFER_SIZE;
        let dfu_version: u16 = 0x011a;
        writer.write(
            DFU_TYPE_FUNCTIONAL,  // bDescriptorType
            &[
                Self::dfu_bm_attributes(),  // bmAttributes
                detach_timeout.to_le_bytes()[0], detach_timeout.to_le_bytes()[1],  // wDetachTimeOut
                transfer_size.to_le_bytes()[0], transfer_size.to_le_bytes()[1], // wTransferSize
                dfu_version.to_le_bytes()[0], dfu_version.to_le_bytes()[1], // bcdDFUVersion
            ])
    }

    fn control_in(&mut self, xfer:ControlIn<B>) {
        let req = xfer.request();

        if !(req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.iface) as u16)
        {
            return;
        }

        match req.request {
            DFU_REQ_GETSTATUS => {
                let status: [u8;6] = [DfuStatusCode::OK as u8,
                                      0,0,0, // poll timeout in milliseconds
                                      self.state as u8,
                                      0]; // iString for status description
                xfer.accept_with(&status).ok();
            },
            _ => {
                xfer.reject().ok();
            },
        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();

        if !(req.request_type == control::RequestType::Class
            && req.recipient == control::Recipient::Interface
            && req.index == u8::from(self.iface) as u16)
        {
            return;
        }

        match req.request {
            DFU_REQ_DETACH => {
                self.timeout = self.ops.allow(req.value);
                if self.timeout.is_some() {
                    self.state = DfuState::AppDetach;
                    xfer.accept().ok();
                } else {
                    xfer.reject().ok();
                }
            },
            _ => { xfer.reject().ok(); },
        }
    }

    fn reset(&mut self) {
        if !T::WILL_DETACH && self.timeout.is_some() {
            self.timeout = None;
            self.ops.detach();
        }
    }
}

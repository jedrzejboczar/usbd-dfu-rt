use usb_device::class_prelude::*;
use usb_device::Result;

const USB_CLASS_APPLICATION_SPECIFIC: u8 = 0xfe;

const DFU_SUBCLASS_FIRMWARE_UPGRADE: u8 = 0x01;

const DFU_PROTOCOL_RUNTIME: u8 = 0x01;

const DFU_TYPE_FUNCTIONAL: u8 = 0x21;
const DFU_WILL_DETACH: u8 = 1 << 3;
const DFU_MANIFESTATION_TOLERANT: u8 = 1 << 2;
const DFU_CAN_UPLOAD: u8 = 1 << 1;
const DFU_CAN_DNLOAD: u8 = 1 << 0;

const DFU_REQ_DETACH: u8 = 0;

/// Implementation of DFU runtime class.
///
/// Implements DFU_DETACH request and will call the [`DfuRuntimeOps::enter`] callback when
/// the request is received.
pub struct DfuRuntimeClass<T: DfuRuntimeOps> {
    dfu_ops: T,
    iface: InterfaceNumber,
    timeout: Option<u16>,
}

/// Trait that defines device-specific operations for [`DfuRuntimeClass`].
pub trait DfuRuntimeOps {
    /// Enter DFU mode.
    ///
    /// This is a callback that will be called after receiving the DFU_DETACH request.
    fn enter(&mut self);

    // TODO: get_time_ms, configurable DFU functional descriptor
}

impl<T: DfuRuntimeOps> DfuRuntimeClass<T> {
    /// Crate new DFU run-time class with the given device-specific implementations.
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>, dfu_ops: T) -> Self {
        Self {
            dfu_ops,
            iface: alloc.interface(),
            timeout: None,
        }
    }
}

impl<T: DfuRuntimeOps, B: UsbBus> UsbClass<B> for DfuRuntimeClass<T> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        writer.iad(
            self.iface,
            1,
            USB_CLASS_APPLICATION_SPECIFIC,
            DFU_SUBCLASS_FIRMWARE_UPGRADE,
            DFU_PROTOCOL_RUNTIME)?;

        writer.interface(
            self.iface,
            USB_CLASS_APPLICATION_SPECIFIC,
            DFU_SUBCLASS_FIRMWARE_UPGRADE,
            DFU_PROTOCOL_RUNTIME)?;

        // Run-Time DFU Functional Descriptor
        let detach_timeout: u16 = 255;
        let transfer_size: u16 = 2048;
        let dfu_version: u16 = 0x011a;
        writer.write(
            DFU_TYPE_FUNCTIONAL,  // bDescriptorType
            &[
                (DFU_WILL_DETACH | DFU_CAN_UPLOAD | DFU_CAN_DNLOAD) & !DFU_MANIFESTATION_TOLERANT,  // bmAttributes
                // 0,  // bmAttributes
                detach_timeout.to_le_bytes()[0], detach_timeout.to_le_bytes()[1],  // wDetachTimeOut
                transfer_size.to_le_bytes()[0], transfer_size.to_le_bytes()[1], // wTransferSize
                dfu_version.to_le_bytes()[0], dfu_version.to_le_bytes()[1], // bcdDFUVersion
            ])
    }

    fn poll(&mut self) {
        // TODO: implement timeout
        if let Some(_timeout) = self.timeout.take() {
            self.dfu_ops.enter();
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
                self.timeout = Some(req.value);
                xfer.accept().ok();
            },
            _ => { xfer.reject().ok(); },
        }
    }
}

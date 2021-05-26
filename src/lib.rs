#![no_std]
#![deny(missing_docs)]

//! Implementation of DFU runtime class
//!
//! DFU stands for Device Firmware Upgrade. DFU defines two USB classes:
//! * DFU mode: used to transfer new firmware and flash the device
//! * Run-time: advertises DFU capability and can be used to change device
//!   mode to DFU upgrade
//!
//! This crate implements DFU run-time class according to UDB DFU class
//! specification Version 1.1a. It implements _only_ the run-time class which
//! means that it only implements DFU_DETACH request. This request means that
//! the device should switch to DFU mode in preparation for firmware upgrade.
//! This usually means rebooting to a DFU-capable bootloader which then handles
//! the upgrade.
//!
//! To use this class user must provide a callback that will perform the
//! transition to DFU mode.

/// DFU runtime class
pub mod class;

pub use crate::class::{DfuRuntimeClass, DfuRuntimeOps};

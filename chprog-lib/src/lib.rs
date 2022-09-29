//! ChProg is a firmware operations library designed
//! for using UART with WCH CH55x series microcontrollers
//!
//! Library was created as Aleksey Evtyushkin's final project
//! in an otus.ru course of the Rust programming language
//!
//! ## Features
//! - Connect chip via UART serial port
//! - Reset chip to bootloader using DTR and RTS lines
//! - Erase flash memory on chip
//! - Detect chip type
//! - Flash firmware file to chip
//! - Verify flashed firmware with file

#[macro_use]
extern crate lazy_static;

pub mod definitions;
pub mod protocol;
pub mod sequence;

use protocol::{Protocol, ProtocolError};
use serial::prelude::*;
use std::time::Duration;

/// Chip firmware operations stucture
pub struct ChProg {
    protocol: Protocol,
}

impl ChProg {
    /// Creates new ChProg instance, opens specified [serial_port]
    /// and do initial serial setup
    pub fn new(serial_port: String) -> Result<Self, ProtocolError> {
        // Try to open serial port
        let port_result = serial::open(&serial_port);
        if port_result.is_err() {
            return Err(ProtocolError::SerialError);
        }

        // Following setup procedure
        let mut port_box = Box::new(port_result.unwrap());

        // Set timeout
        port_box.set_timeout(Duration::from_millis(150)).ok();

        // Set port settings
        let port_setup = port_box.reconfigure(&|settings| {
            settings.set_baud_rate(serial::Baud57600).ok();
            settings.set_char_size(serial::Bits8);
            settings.set_parity(serial::ParityNone);
            settings.set_stop_bits(serial::Stop1);
            settings.set_flow_control(serial::FlowNone);

            Ok(())
        });

        if port_setup.is_err() {
            return Err(ProtocolError::SerialError);
        }

        // Return self
        Ok(ChProg {
            protocol: Protocol::new(port_box),
        })
    }

    // High level functions
    /// Execute chip reset sequence
    pub fn reset(&mut self) {
        self.protocol.chip_reset();
    }

    /// Erase chip flash memory
    pub fn erase(&mut self) -> Result<(), ProtocolError> {
        self.protocol.erase()
    }

    /// Detect chip
    pub fn detect(&mut self) -> Result<(), ProtocolError> {
        self.protocol.bootloader_detect();
        self.protocol.chip_detect()
    }

    /// Write flash firmware with specified [filename]
    pub fn flash(&mut self, filename: String) -> Result<(), ProtocolError> {
        self.protocol.write(filename)
    }

    /// Verify flash firmware with specified [filename]
    pub fn verify(&mut self, filename: String) -> Result<(), ProtocolError> {
        self.protocol.verify(filename)
    }
}

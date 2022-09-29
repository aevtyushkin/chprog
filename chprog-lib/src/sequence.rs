//! ChProg sequence file
//!
//! Bootloader versions have different byte sequences for managing microcontroller

use std::collections::HashMap;

#[derive(Clone, Copy, Hash, PartialEq)]
pub enum Bootloader {
    Unknown,
    V1,
    V2,
}

impl Eq for Bootloader {}

#[derive(Clone, Copy)]
pub struct Sequence {
    pub chip_detect: &'static [u8],
    pub bootloader_exit: &'static [u8],
    pub flash_erase: &'static [u8],
    pub mode_write: &'static [u8],
    pub mode_verify: &'static [u8],
    pub config_read: &'static [u8],
    pub config_write: &'static [u8],
}

lazy_static! {
    pub static ref SEQUENCES: HashMap<Bootloader, Sequence> = [
        (
            Bootloader::V1,
            Sequence {
                chip_detect: &[
                    0xA2, 0x13, 0x55, 0x53, 0x42, 0x20, 0x44, 0x42, 0x47, 0x20, 0x43, 0x48, 0x35,
                    0x35, 0x39, 0x20, 0x26, 0x20, 0x49, 0x53, 0x50, 0x00
                ],
                bootloader_exit: &[0xA5, 0x02, 0x01, 0x00],
                flash_erase: &[0xA6, 0x04, 0x00, 0x00, 0x00, 0x00],
                mode_write: &[0xA8],
                mode_verify: &[0xA7],
                config_read: &[0xBB, 0x00],
                config_write: &[],
            }
        ),
        (
            Bootloader::V2,
            Sequence {
                chip_detect: &[
                    0xA1, 0x12, 0x00, 0x59, 0x11, 0x4D, 0x43, 0x55, 0x20, 0x49, 0x53, 0x50, 0x20,
                    0x26, 0x20, 0x57, 0x43, 0x48, 0x2E, 0x43, 0x4E
                ],
                bootloader_exit: &[0xA2, 0x01, 0x00, 0x01],
                flash_erase: &[0xA4, 0x01, 0x00, 0x00],
                mode_write: &[0xA5],
                mode_verify: &[0xA6],
                config_read: &[0xA7, 0x02, 0x00, 0x1F, 0x00],
                config_write: &[
                    0xA8, 0x0E, 0x00, 0x07, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x03, 0x00, 0x00, 0x00,
                    0xFF, 0x4E, 0x00, 0x00
                ],
            }
        ),
    ]
    .iter()
    .copied()
    .collect();
}

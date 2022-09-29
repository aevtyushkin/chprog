//! ChProg definitions file
//!
//! Each microcontroller type have variables concerning memory capacity and boot parameters

use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Definition {
    pub flash_blocks: u8,
    pub erase_blocks: u8,
    pub boot_address: u32,
}

lazy_static! {
    pub static ref DEFINITIONS: HashMap<u8, Definition> = [
        (
            0x51, // CH551
            Definition {
                flash_blocks: 10,
                erase_blocks: 10,
                boot_address: 0x3800,
            }
        ),
        (
            0x52, // CH552
            Definition {
                flash_blocks: 16,
                erase_blocks: 14,
                boot_address: 0x3800,
            }
        ),
        (
            0x53, // CH553
            Definition {
                flash_blocks: 10,
                erase_blocks: 10,
                boot_address: 0x3800,
            }
        ),
        (
            0x54, // CH554
            Definition {
                flash_blocks: 16,
                erase_blocks: 14,
                boot_address: 0x3800,
            }
        ),
        (
            0x58, // CH558
            Definition {
                flash_blocks: 40,
                erase_blocks: 32,
                boot_address: 0xF400,
            }
        ),
        (
            0x59, // CH559
            Definition {
                flash_blocks: 64,
                erase_blocks: 60,
                boot_address: 0xF400,
            }
        ),
    ]
    .iter()
    .copied()
    .collect();
}

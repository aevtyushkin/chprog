//! ChProg protocol file
//!
//! Basic logic of working with the microcontroller

use super::definitions::DEFINITIONS;
use super::sequence::{Bootloader, SEQUENCES};
use rand::Rng;
use serial::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::time::Duration;
use thiserror::Error;

/// Firmware flashing mode
pub enum Mode {
    Write,
    Verify,
}

/// Possible errors while using library
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Preamble mismatch")]
    PreableMismatch,
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    #[error("Serial timeout")]
    SerialTimeout,
    #[error("Serial error")]
    SerialError,
    #[error("File access error")]
    FileAccessError,
    #[error("File format error")]
    FileFormatError,
    #[error("Bootloader unknown")]
    BootloaderUnknown,
    #[error("Chip unknown")]
    ChipUnknown,
}

/// For storing MCU information
pub struct ChipInfo {
    pub bootloader: Bootloader,
    pub chip_id: u8,
}

// Current state
pub struct Protocol {
    chip_info: ChipInfo,
    port: Box<dyn SerialPort>,
    pkt_buffer: [u8; Self::PACKET_MAXLEN],
    bootkey: [u8; 8],
}

impl Protocol {
    /// Maximum request length
    pub const PACKET_MAXLEN: usize = 256;

    /// Create new protocol instance with initial values
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        Protocol {
            chip_info: ChipInfo {
                bootloader: Bootloader::Unknown,
                chip_id: 0,
            },
            port,
            pkt_buffer: [0; Self::PACKET_MAXLEN],
            bootkey: [0; 8],
        }
    }

    /// Default write firmware procedure
    pub fn write(&mut self, filename: String) -> Result<(), ProtocolError> {
        if self.chip_info.bootloader == Bootloader::Unknown {
            // Detect bootloader
            self.bootloader_detect();

            if self.chip_info.bootloader == Bootloader::Unknown {
                return Err(ProtocolError::BootloaderUnknown);
            }
        }

        if self.chip_info.chip_id == 0 {
            // Identify chip
            self.chip_detect()?;
        }

        // Erase chip
        self.erase()?;

        // Write file
        self.flash_file(filename.clone(), Mode::Write)?;

        // Verify file
        self.flash_file(filename, Mode::Verify)?;

        // Exit bootloader
        self.bootloader_exit()?;

        Ok(())
    }

    /// Reset MCU to bootloader
    pub fn chip_reset(&mut self) {
        // Sleep 0.01
        std::thread::sleep(Duration::from_millis(10));

        // Set RST(DTR line) & BOOT(RTS line)
        self.port.set_dtr(true).ok();
        self.port.set_rts(true).ok();

        // Sleep for 0.15
        std::thread::sleep(Duration::from_millis(150));

        // Unset RST(DTR line)
        self.port.set_dtr(false).ok();

        // Sleep 0.1 & unset BOOT(RTS line) to start bootloader
        std::thread::sleep(Duration::from_millis(100));
        self.port.set_rts(false).ok();

        // Wait 0.25 to settle bootloader
        std::thread::sleep(Duration::from_millis(250));
    }

    /// Verify firmware on MCU with firmware loaded from file speficied in *filename*
    pub fn verify(&mut self, filename: String) -> Result<(), ProtocolError> {
        // Detect bootloader
        self.bootloader_detect();

        // Identify chip
        self.chip_detect()?;

        // Verify file
        self.flash_file(filename, Mode::Verify)?;

        Ok(())
    }

    /// Erase MCU flash
    pub fn erase(&mut self) -> Result<(), ProtocolError> {
        match self.chip_info.bootloader {
            Bootloader::V1 => {
                // Send request
                if self
                    .request_send(SEQUENCES[&Bootloader::V1].flash_erase)
                    .is_ok()
                {
                    let device_erase_size = DEFINITIONS[&self.chip_info.chip_id].erase_blocks;

                    // Erase each block
                    for erase_block_index in 0..device_erase_size {
                        let erase_block_request: [u8; 4] =
                            [0xA9, 0x02, 0x00, (erase_block_index * 4) as u8];

                        println!("Erasing block: {}", erase_block_index);

                        match self.request_send(&erase_block_request) {
                            Ok(reply) => {
                                if reply[0] != 0x00 {
                                    //println!("ERROR: Erase failed");
                                    return Err(ProtocolError::ChipUnknown);
                                }
                            }
                            Err(err) => return Err(err),
                        }
                    }

                    println!("Flash erased");
                    return Ok(());
                }
            }
            Bootloader::V2 => {
                let device_erase_size = DEFINITIONS[&self.chip_info.chip_id].erase_blocks;
                let mut device_erase_sequence: [u8; 4] = [0; 4];

                // Copy sequence
                #[allow(clippy::manual_memcpy)]
                for seq_index in 0..device_erase_sequence.len() {
                    device_erase_sequence[seq_index] =
                        SEQUENCES[&Bootloader::V2].flash_erase[seq_index];
                }

                // Insert erase block value from definitions
                device_erase_sequence[3] = device_erase_size;

                match self.request_send(&device_erase_sequence) {
                    Ok(reply) => {
                        if reply[4] != 0x00 {
                            //println!("ERROR: Erase failed");
                            return Err(ProtocolError::ChipUnknown);
                        }
                    }
                    Err(err) => return Err(err),
                }

                println!("Flash erased");
                return Ok(());
            }
            Bootloader::Unknown => {
                //println!("Unknown bootloader");
                return Err(ProtocolError::BootloaderUnknown);
            }
        }

        Err(ProtocolError::ChipUnknown)
    }

    /// Exit from MCU bootloader
    pub fn bootloader_exit(&mut self) -> Result<(), ProtocolError> {
        match self.chip_info.bootloader {
            Bootloader::Unknown => {
                //println!("Unknown bootloader");
                return Err(ProtocolError::BootloaderUnknown);
            }
            _ => {
                // Send request bootloader exit
                if let Err(err) =
                    self.request_send(SEQUENCES[&self.chip_info.bootloader].bootloader_exit)
                {
                    //println!("Error while sending request");
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    /// Send *sequence* to MCU
    fn request_send(&mut self, sequence: &[u8]) -> Result<&[u8], ProtocolError> {
        let mut request: [u8; Self::PACKET_MAXLEN] = [0; Self::PACKET_MAXLEN];
        let mut request_checksum: u8 = 0;
        let mut reply_checksum: u8 = 0;

        // Clear buffer
        self.pkt_buffer = [0; Self::PACKET_MAXLEN];

        // Calculate sequence length
        let sequence_len = sequence.len();

        // Add preamble
        request[0] = 0x57;
        request[1] = 0xAB;

        // Copy sequence
        for seq_index in 0..sequence_len {
            request[2 + seq_index] = sequence[seq_index];

            // Calculate request checksum
            request_checksum = request_checksum.overflowing_add(sequence[seq_index]).0;
        }

        // Insert checksum
        request[2 + sequence_len] = request_checksum;

        // Write serial
        self.port.write(&request[..=(2 + sequence_len)]).ok();

        // Read reply from serial until timeout
        let mut reply_len = 0;
        while self
            .port
            .read_exact(&mut self.pkt_buffer[reply_len..reply_len + 1])
            .is_ok()
        {
            reply_len += 1;
        }

        // Process packet if remote device replied
        if reply_len == 0 {
            //println!("ERROR: Serial read timeout");
            return Err(ProtocolError::SerialTimeout);
        }

        // Check preamble
        if (self.pkt_buffer[0] != 0x55) || (self.pkt_buffer[1] != 0xAA) {
            // Wrong preamble
            //println!("ERROR: Wrong preamble");
            return Err(ProtocolError::PreableMismatch);
        }

        // Calc reply checksum
        for reply_index in 2..reply_len - 1 {
            reply_checksum = reply_checksum
                .overflowing_add(self.pkt_buffer[reply_index])
                .0;
        }

        if reply_checksum != self.pkt_buffer[reply_len - 1] {
            // Checksum error
            // println!(
            //     "ERROR: Checksum error {} != {}",
            //     reply_checksum,
            //     self.pkt_buffer[reply_len - 1]
            // );
            return Err(ProtocolError::ChecksumMismatch);
        }

        Ok(&self.pkt_buffer[2..reply_len - 1]) // Exclude preamble and checksum
    }

    /// Detect bootloader on a connected chip
    pub fn bootloader_detect(&mut self) {
        // Check if bootloader is already detected
        if self.chip_info.bootloader != Bootloader::Unknown {
            return;
        }

        // Send chip detect request
        if let Ok(reply) = self.request_send(SEQUENCES[&Bootloader::V2].chip_detect) {
            if reply.len() == 2 {
                //println!("Detected v1 bootloader");
                self.chip_info.bootloader = Bootloader::V1;
                return;
            }

            //println!("Detected v2 bootloader");
            self.chip_info.bootloader = Bootloader::V2;
            return;
        }

        println!("ERROR: Bootloader not detected");
        self.chip_info.bootloader = Bootloader::Unknown;
    }

    /// Detect connected chip
    pub fn chip_detect(&mut self) -> Result<(), ProtocolError> {
        match self.chip_info.bootloader {
            Bootloader::V1 => {
                // Identify chip
                let reply = self.request_send(SEQUENCES[&Bootloader::V1].chip_detect)?;
                if reply.len() != 2 {
                    // Unknown chip
                    return Err(ProtocolError::ChipUnknown);
                }

                self.chip_info.chip_id = reply[0];
                println!("Detected chip model: CH5{:02X}", self.chip_info.chip_id);

                // Read config
                let reply = self.request_send(SEQUENCES[&Bootloader::V1].config_read)?;
                if reply.len() != 2 {
                    // Unknown bootloader
                    return Err(ProtocolError::BootloaderUnknown);
                }

                println!(
                    "Detected bootloader version: {}.{}",
                    reply[0] >> 4,
                    reply[1] & 0x0F
                );
            }
            Bootloader::V2 => {
                // Random key is a way(guess) to protecting against brute-force flash dump
                let mut rng = rand::thread_rng();

                // Identify chip
                let reply = self.request_send(SEQUENCES[&Bootloader::V2].chip_detect)?;
                if reply.len() != 6 {
                    // Unknown chip
                    return Err(ProtocolError::ChipUnknown);
                }

                self.chip_info.chip_id = reply[4];
                println!("Detected chip model: CH5{:02X}", self.chip_info.chip_id);

                // Read config
                let reply = self.request_send(SEQUENCES[&Bootloader::V2].config_read)?;
                if reply.len() != 30 {
                    // Unknown bootloader
                    println!("ERROR: Unexpected bootloader reply length");
                    return Err(ProtocolError::BootloaderUnknown);
                }

                println!(
                    "Detected bootloader version: {}.{}{}",
                    reply[19], reply[20], reply[21]
                );

                // Key input
                let mut request: [u8; Self::PACKET_MAXLEN] = [0; Self::PACKET_MAXLEN];
                request[0] = 0xA3;
                request[1] = 0x30;
                request[2] = 0x00;

                // Checksum
                let mut checksum: u8 = 0;
                for reply_byte in reply[22..26].iter() {
                    checksum = checksum.overflowing_add(*reply_byte).0;
                }

                // Random sequence
                for req_index in 0..48 {
                    let random_byte: u8 = rng.gen();
                    request[3 + req_index] = random_byte;
                }

                // Calculate the key from the random list
                self.bootkey[0] = request[(3 + ((request[1] / 7) as u8 * 4)) as usize] ^ checksum;
                self.bootkey[1] = request[(3 + ((request[1] / 5) as u8)) as usize] ^ checksum;
                self.bootkey[2] = request[(3 + ((request[1] / 7) as u8)) as usize] ^ checksum;
                self.bootkey[3] = request[(3 + ((request[1] / 7) as u8 * 6)) as usize] ^ checksum;
                self.bootkey[4] = request[(3 + ((request[1] / 7) as u8 * 3)) as usize] ^ checksum;
                self.bootkey[5] = request[(3 + ((request[1] / 5) as u8 * 3)) as usize] ^ checksum;
                self.bootkey[6] = request[(3 + ((request[1] / 7) as u8 * 5)) as usize] ^ checksum;
                self.bootkey[7] = self.chip_info.chip_id.overflowing_add(self.bootkey[0]).0;

                // Get key checksum
                let mut key_checksum: u8 = 0;
                for key_index in 0..8 {
                    key_checksum = key_checksum.overflowing_add(self.bootkey[key_index]).0;
                }

                // Send request
                let key_reply = self.request_send(&request[0..51])?;

                if key_reply[4] != key_checksum {
                    // println!(
                    //     "ERROR: Key checksum error, expected {} got {}",
                    //     key_checksum, key_reply[4]
                    // );

                    return Err(ProtocolError::BootloaderUnknown);
                }

                //println!("Checksum: 0x{:02X}", checksum);
                //println!("Generated bootkey: {:02X?}", self.bootkey);
            }
            Bootloader::Unknown => {
                // Unknown bootloader
                //println!("ERROR: Unknown bootloader");
                return Err(ProtocolError::BootloaderUnknown);
            }
        }

        Ok(())
    }

    // Send file to MCU flash
    fn flash_file(&mut self, filename: String, mode: Mode) -> Result<(), ProtocolError> {
        if self.chip_info.bootloader == Bootloader::Unknown {
            //println!("ERROR: Unknown bootloader cannot flash");
            return Err(ProtocolError::BootloaderUnknown);
        }

        // Try to open specified filename
        let maybe_fd = File::open(filename);
        if maybe_fd.is_err() {
            //println!("ERROR: Cannot open specified file to flash");
            return Err(ProtocolError::FileAccessError);
        }

        // File opened, we could safely unwrap here
        let fd = maybe_fd.unwrap();
        let mut reader = BufReader::new(fd);
        let mut file_buffer = Vec::new();

        // Read file into u8 vector.
        if reader.read_to_end(&mut file_buffer).is_err() {
            //println!("ERROR: Cannot read specified file to flash");
            return Err(ProtocolError::FileAccessError);
        }

        // Check file size
        let filesize = file_buffer.len();
        println!("Firmware filesize: {} bytes", filesize);

        if filesize < 32 {
            //println!("ERROR: Firmware bin file possibly corrupt.");
            return Err(ProtocolError::FileFormatError);
        }

        // Make the buffer length to be on 8 bytes boundary
        let mut len_bound = filesize;
        len_bound = len_bound + (len_bound % 8);

        // Get mode op code
        let mode_code = match mode {
            Mode::Verify => {
                //println!("Verifying flash...");
                SEQUENCES[&self.chip_info.bootloader].mode_verify[0]
            }
            Mode::Write => {
                //println!("Writting flash...");
                SEQUENCES[&self.chip_info.bootloader].mode_write[0]
            }
        };

        // Form packet
        let mut cur_addr = 0;
        let mut bytes_to_send = filesize;
        while cur_addr < len_bound {
            let mut pkt_length;
            let mut packet: [u8; 64] = [0; 64];

            match self.chip_info.bootloader {
                Bootloader::V1 => {
                    // Calc packet length
                    if bytes_to_send >= 60 {
                        pkt_length = 60;
                    } else {
                        pkt_length = bytes_to_send;
                    }

                    // Fill header
                    packet[0] = mode_code;
                    packet[1] = (pkt_length & 0xFF) as u8;
                    packet[2] = (cur_addr & 0xFF) as u8;
                    packet[3] = ((cur_addr >> 8) & 0xFF) as u8;

                    // Copy contents
                    packet[4..(pkt_length + 4)]
                        .copy_from_slice(&file_buffer[cur_addr..(pkt_length + cur_addr)]);

                    // Send data
                    let reply = self.request_send(&packet[..])?;
                    cur_addr += pkt_length;
                    bytes_to_send -= pkt_length;

                    if reply[0] != 0x00 {
                        // println!(
                        //     "ERROR: Error while sending data: Write failed at address 0x{:04X}",
                        //     cur_addr
                        // );
                        return Err(ProtocolError::SerialError);
                    }
                }
                Bootloader::V2 => {
                    // Calc packet length
                    if bytes_to_send >= 56 {
                        pkt_length = 56;
                    } else {
                        pkt_length = bytes_to_send;
                    }

                    // Fill header
                    packet[0] = mode_code;
                    packet[1] = ((pkt_length + (pkt_length % 8) + 5) & 0xFF) as u8;
                    packet[2] = 0x00;
                    packet[3] = (cur_addr & 0xFF) as u8;
                    packet[4] = ((cur_addr >> 8) & 0xFF) as u8;
                    packet[5] = 0x00;
                    packet[6] = 0x00;
                    packet[7] = (bytes_to_send & 0xFF) as u8;

                    // Copy contents
                    packet[8..(pkt_length + 8)]
                        .copy_from_slice(&file_buffer[cur_addr..(pkt_length + cur_addr)]);

                    // Update packet length to make on 8 bytes boundary
                    pkt_length = pkt_length + (pkt_length % 8);

                    // XOR data with the bootkey
                    for buffer_index in 0..pkt_length {
                        packet[buffer_index + 8] ^= self.bootkey[buffer_index & 0x07];
                    }

                    println!("Processing at address: 0x{:04X}", cur_addr);

                    // Send data
                    let reply = self.request_send(&packet[..pkt_length + 8])?;
                    if (reply[4] != 0x00) && (reply[4] != 0xFE) {
                        // println!(
                        //     "ERROR: Error while sending data: Failed at address {}",
                        //     cur_addr
                        // );
                        return Err(ProtocolError::SerialError);
                    }

                    cur_addr += pkt_length;
                    if bytes_to_send >= pkt_length {
                        bytes_to_send -= pkt_length;
                    } else {
                        //println!("Complete!");
                        return Ok(());
                    }
                }
                Bootloader::Unknown => {
                    //println!("Unknown bootloader");
                    return Err(ProtocolError::BootloaderUnknown);
                }
            }
        }

        //println!("Writing success");
        Ok(())
    }
}

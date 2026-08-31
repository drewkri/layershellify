use std::fmt::Display;

use bytemuck::from_bytes;
use log::{debug, warn};

/// Struct representing a message in Wayland and containing all its information.
pub struct WaylandMessage {
    size: u16,
    opcode: u16,
    obj_id: u32,
    arguments_data: Vec<u8>,
}

impl Display for WaylandMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Size: {}, Opcode: {}, Object ID {}\nRaw Args Data: {:?}",
            self.size, self.opcode, self.obj_id, self.arguments_data
        )
    }
}

pub fn decode_messages(bytes: &[u8]) -> Vec<WaylandMessage> {
    let mut vec = Vec::new();
    let mut bytes_remaining = bytes.len();
    // will be redefined to move forward in the slice
    let mut bytes = bytes;
    loop {
        if bytes_remaining == 0 {
            break;
        }
        if bytes_remaining < 8 {
            warn!(
                "Malformed wayland message, less than 8 bytes long. Message: {:?}",
                bytes
            );
            break;
        }
        // These will panic on failure, but casting bytes to an integer should never fail.
        let obj_id: u32 = *from_bytes(&bytes[0..4]);
        let opcode: u16 = *from_bytes(&bytes[4..6]);
        let size: u16 = *from_bytes(&bytes[6..8]);
        let usize_size = size as usize;
        if usize_size > bytes_remaining {
            warn!(
                "Message size greater than number of bytes read; message was: {:?}",
                bytes
            );
            break;
        }
        if usize_size < 8 {
            warn!("Malformed message, size < 8; message was: {:?}", bytes);
            break;
        }
        let arguments_data = bytes[8..usize_size].iter().cloned().collect();
        if let Some(num) = bytes_remaining.checked_sub(usize_size) {
            bytes_remaining = num;
        } else {
            warn!("Malformed wayland message or read, fewer bytes read than size of message.");
            break;
        }
        bytes = &bytes[usize_size..];
        vec.push(WaylandMessage {
            size,
            opcode,
            obj_id,
            arguments_data,
        });
    }
    vec
}

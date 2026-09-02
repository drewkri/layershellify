use std::{fmt::Display, u32};

use bytemuck::from_bytes;
use log::{debug, warn};

use crate::{ProxyPhase, State, util::message_header};

const LAYERSHELLIFY_STRING: [u8; 44] = [
    39, 0, 0, 0, // size (16)
    105, 111, 46, 103, 105, 116, 104, 117, 98, 46, 68, 114, 101, 119, 67, 111, 100, 101, 115, 66,
    97, 100, 108, 121, 46, 76, 97, 121, 101, 114, 115, 104, 101, 108, 108, 105, 102, 121, 0, 0,
];
const LAYER_SHELL_STRING_LEN: u32 = 20;

/// Template used to bind to zwlr_layer_shell_v1. Prefix
/// with the u32 name advertised by the compositor.
const LAYER_SHELL_BIND_TEMPLATE: [u8; 28] = [
    /* name must go here (u32) */
    20, 0, 0, 0, // String size
    122, 119, 108, 114, 95, 108, 97, 121, 101, 114, 95, // String + null terminator
    115, 104, 101, 108, 108, 95, 118, 49, 0, // String + null terminator (continued)
    5, 0, 0, 0, // Version
       /* New ID goes here */
];

/// "xdg_wm_base" in bytes, with a null terminator
const XDG_WM_BASE_STRING: [u8; 12] = [120, 100, 103, 95, 119, 109, 95, 98, 97, 115, 101, 0];

/// Struct representing a message in Wayland and containing all its information.
pub struct WaylandMessage {
    pub size: u16,
    pub opcode: u16,
    pub obj_id: u32,
    pub arguments_data: Vec<u8>,
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

/// Returned by the callback in `scan_messages` to determine what to do with the message.
pub enum MessageOperation {
    Drop,             // Drop the message; do not copy it over
    Keep,             // Copy over the message unchanged
    Replace(Vec<u8>), // Replace with this new message.
}

pub fn scan_requests(bytes: &[u8], state: &mut State) -> Vec<u8> {
    scan_messages(bytes, state, |bytes, state, obj_id, opcode| {
        match state.phase {
            ProxyPhase::Binding {
                registry_id,
                advertised_layer_shell_name,
                layer_shell_id,
            } => {
                // wl_registry::bind
                if let Some(id) = registry_id
                    && obj_id == id
                    && opcode == 0
                {
                    // Ignore name, we're interested in the rest of it
                    // technically I'm not clear on why you can't just send the name and
                    // the new id, but every client i've seen thus far sends the new id with
                    // defined interface so I will assume they all do that.
                    let str_size: u32 = *from_bytes(&bytes[12..16]);
                    // size of xdg_wm_base + null terminator
                    if str_size == XDG_WM_BASE_STRING.len() as u32 {
                        // Verify the string is actually that
                        if bytes[16..28] == XDG_WM_BASE_STRING {
                            // Get the new id
                            // The next 4 bytes are version, then id
                            // If the client is binding to globals, we should've already sent our layer shell bind request
                            // since the server must've advertised globals.
                            let xdg_wm_id: u32 = *from_bytes(&bytes[32..36]);
                            let mut message = message_header(
                                id,
                                0,
                                8 + 4 + LAYER_SHELL_BIND_TEMPLATE.len() as u16 + 4,
                            );
                            let name = advertised_layer_shell_name
                                .expect("Server did not advertise layer shell global.");
                            message.extend_from_slice(&name.to_le_bytes());
                            message.extend_from_slice(&LAYER_SHELL_BIND_TEMPLATE);
                            message.extend_from_slice(&xdg_wm_id.to_le_bytes());
                            debug!("[STATE CHANGE] Bound layer shell, now listening.");
                            state.phase = ProxyPhase::Listening {
                                layer_shell_id: xdg_wm_id,
                                layer_surface_id: None,
                            };
                            // Replace with layer shell bind
                            return MessageOperation::Replace(message);
                        }
                    }
                }
                // wl_display::get_registry(new_id)
                // We still need to handle this since the client may try binding from
                // a different registry
                // However I have yet to see a client bind xdg_wm_base twice before
                // creating a window so I think we're fine there.
                // I also have yet to see a client return to a previously created registry and then bind
                // xdg_wm_base and cannot imagine that happening so I have elected not to cover that case.
                else if obj_id == 1 && opcode == 1 {
                    let new_reg_id = *from_bytes(&bytes[8..12]);
                    state.phase = ProxyPhase::Binding {
                        registry_id: Some(new_reg_id),
                        advertised_layer_shell_name,
                        layer_shell_id,
                    };
                }
                MessageOperation::Keep
            }
            ProxyPhase::Listening {
                layer_shell_id,
                layer_surface_id,
            } => {
                // xdg_wm_base::get_xdg_surface
                if obj_id == layer_shell_id && opcode == 2 {
                    let new_id: u32 = *from_bytes(&bytes[8..12]);
                    let surface_id: u32 = *from_bytes(&bytes[12..16]);

                    // Create layer shell
                    let mut message = message_header(layer_shell_id, 0, 68);
                    message.extend_from_slice(&new_id.to_le_bytes()); // new id
                    message.extend_from_slice(&surface_id.to_le_bytes()); // wl_surface
                    message.extend_from_slice(&0u32.to_le_bytes()); // output (null)
                    message.extend_from_slice(&3u32.to_le_bytes()); // layer (overlay)
                    // message.extend_from_slice(&0u32.to_le_bytes()); // string
                    message.extend_from_slice(&LAYERSHELLIFY_STRING);
                    debug!("Message length was: {}", message.len());

                    // Set anchors
                    message.extend_from_slice(&message_header(new_id, 1, 12));
                    message.extend_from_slice(&4u32.to_le_bytes());

                    // Set size
                    message.extend_from_slice(&message_header(new_id, 0, 16));
                    message.extend_from_slice(&128u32.to_le_bytes());
                    message.extend_from_slice(&128u32.to_le_bytes());

                    // Set margin
                    message.extend_from_slice(&message_header(new_id, 3, 24));
                    message.extend_from_slice(&8u32.to_le_bytes());
                    message.extend_from_slice(&8u32.to_le_bytes());
                    message.extend_from_slice(&8u32.to_le_bytes());
                    message.extend_from_slice(&8u32.to_le_bytes());

                    // Set keyboard interactivity
                    message.extend_from_slice(&message_header(new_id, 4, 12));
                    message.extend_from_slice(&2u32.to_le_bytes());

                    // Update state
                    debug!("Sending layer surface creation messages.");
                    state.phase = ProxyPhase::Listening {
                        layer_shell_id,
                        layer_surface_id: Some(new_id),
                    };

                    MessageOperation::Replace(message)
                    // MessageOperation::Keep
                } else {
                    MessageOperation::Keep
                }
            }
        }
    })
}

pub fn scan_events(bytes: &[u8], state: &mut State) -> Vec<u8> {
    scan_messages(bytes, state, |bytes, state, obj_id, opcode| {
        match state.phase {
            ProxyPhase::Binding {
                registry_id,
                advertised_layer_shell_name: _,
                layer_shell_id,
            } => {
                // wl_registry::global
                if let Some(id) = registry_id
                    && obj_id == id
                    && opcode == 0
                {
                    let str_len: u32 = *from_bytes(&bytes[12..16]);
                    if str_len == LAYER_SHELL_STRING_LEN {
                        if LAYER_SHELL_BIND_TEMPLATE[4..(4 + LAYER_SHELL_STRING_LEN as usize)]
                            == bytes[16..(16 + LAYER_SHELL_STRING_LEN as usize)]
                        {
                            // This is the layer shell advertisement.
                            let name = *from_bytes(&bytes[8..12]);
                            debug!("Found zwlr_layer_shell_v1 advertised, will bind later.");
                            state.phase = ProxyPhase::Binding {
                                registry_id,
                                layer_shell_id,
                                advertised_layer_shell_name: Some(name),
                            };
                        }
                    }
                }
                MessageOperation::Keep
            }
            ProxyPhase::Listening {
                layer_shell_id,
                layer_surface_id,
            } => MessageOperation::Keep,
        }
    })
}

pub fn scan_messages(
    bytes: &[u8],
    state: &mut State,
    callback: fn(&[u8], &mut State, u32, u16) -> MessageOperation,
) -> Vec<u8> {
    // Used with_capacity since we expect the output
    // number of bytes should be pretty close to the input number
    let mut vec = Vec::with_capacity(bytes.len());
    let mut bytes_remaining = bytes.len();
    // will be redefined to move forward in the slice
    let mut bytes = bytes;
    loop {
        // Check if there is still a valid message left in the buffer
        if bytes_remaining == 0 {
            break;
        }
        if bytes_remaining < 8 {
            warn!(
                "Malformed wayland message, less than 8 bytes long. Message: {:?}",
                bytes
            );
            // copy remainder to dest anyway, in case it was important.
            vec.extend_from_slice(bytes);
            break;
        }
        // These will panic on failure, but casting bytes to an integer should never fail.
        // These are the basic pieces of information that need to be scanned first
        let obj_id: u32 = *from_bytes(&bytes[0..4]);
        let opcode: u16 = *from_bytes(&bytes[4..6]);
        let size: u16 = *from_bytes(&bytes[6..8]);
        // TODO: FIX THIS INSTEAD
        if size == 0 {
            vec.extend_from_slice(bytes);
            break;
        }
        let usize_size = size as usize;
        // Verify the message size is OK
        if let Some(num) = bytes_remaining.checked_sub(usize_size) {
            bytes_remaining = num;
        } else {
            warn!("Malformed wayland message or bad read, fewer bytes read than size of message.");
            // copy to dest anyways
            vec.extend_from_slice(bytes);
            break;
        }

        // Let the caller decide what to do with each message
        match callback(bytes, state, obj_id, opcode) {
            MessageOperation::Drop => {
                debug!(
                    "[DROPPED MESSAGE] Object: {}, Opcode: {}, Size: {}",
                    obj_id, opcode, size
                );
            }
            MessageOperation::Keep => {
                debug!(
                    "[KEPT MESSAGE] Object: {}, Opcode: {}, Size: {}\nFull Message: {:?}",
                    obj_id,
                    opcode,
                    size,
                    &bytes[0..usize_size],
                );
                vec.extend_from_slice(&bytes[0..usize_size]);
            }
            MessageOperation::Replace(items) => {
                debug!(
                    "[REPLACED MESSAGE] Object: {}, Opcode: {}, Size: {}\nReplaced with: {:?}",
                    obj_id, opcode, size, items
                );
                vec.extend_from_slice(&items);
            }
        }

        bytes = &bytes[usize_size..];
    }
    vec
}

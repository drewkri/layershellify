use std::{fmt::Display, u32};

use bytemuck::from_bytes;
use log::{debug, warn};

use crate::{
    State,
    util::{
        LAYER_SHELL_BIND_TEMPLATE, LAYER_SHELL_STRING_LEN, LAYERSHELLIFY_STRING,
        XDG_WM_BASE_STRING, ZXDG_DECORATION_MANAGER_STRING, bind_layer_shell,
        create_useless_object, message_header,
    },
};

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
        if state.client_blacklist_ids.contains(&obj_id) {
            MessageOperation::Drop
        }
        // wl_registry::bind
        else if let Some(id) = state.registry_id
            && obj_id == id
            && opcode == 0
        {
            // Ignore name, we're interested in the rest of it
            // technically I'm not clear on why you can't just send the name and
            // the new id, but every client i've seen thus far sends the new id with
            // defined interface so I will assume they all do that.
            let str_size: u32 = *from_bytes(&bytes[12..16]);
            // size of xdg_wm_base + null terminator
            if str_size == XDG_WM_BASE_STRING.len() as u32
                && state.layer_shell_id.is_none()
                && bytes[16..28] == XDG_WM_BASE_STRING
            {
                // Get the new id
                // The next 4 bytes are version, then id
                // If the client is binding to globals, we should've already sent our layer shell bind request
                // since the server must've advertised globals.
                let xdg_wm_id: u32 = *from_bytes(&bytes[32..36]);
                // The client will only start binding after the server advertises globals.
                let name = state
                    .advertised_layer_shell_name
                    .expect("Server did not advertise layer shell global.");
                let message = bind_layer_shell(id, name, xdg_wm_id);
                debug!("Bound layer shell.");
                state.layer_shell_id = Some(xdg_wm_id);

                // Replace with layer shell bind
                MessageOperation::Replace(message)
            } else if str_size == ZXDG_DECORATION_MANAGER_STRING.len() as u32
                && bytes[16..16 + ZXDG_DECORATION_MANAGER_STRING.len()]
                    == ZXDG_DECORATION_MANAGER_STRING
            {
                let id: u32 = *from_bytes(
                    // Size of arguments added for clarity
                    &bytes[16 + ZXDG_DECORATION_MANAGER_STRING.len() + 1 + 4
                        ..16 + ZXDG_DECORATION_MANAGER_STRING.len() + 1 + 8],
                );
                debug!("Found zxdg_decoration_manager_v1");
                state.xdg_decoration_id = Some(id);
                MessageOperation::Keep
            } else {
                MessageOperation::Keep
            }
        }
        // wl_display::get_registry(new_id)
        else if obj_id == 1 && opcode == 1 {
            let new_reg_id = *from_bytes(&bytes[8..12]);
            state.registry_id = Some(new_reg_id);
            MessageOperation::Keep
        } else if let Some(id) = state.layer_surface_id
            && obj_id == id
        {
            match opcode {
                // destroy
                0 => {
                    let mut vec: Vec<u8> = bytes[0..8].iter().cloned().collect();
                    // swap to layer shell destroy opcode
                    vec[4] = 1;
                    MessageOperation::Replace(vec)
                }
                // get_toplevel
                1 => {
                    // We need to replace the toplevel with a BS object
                    // so that there isn't a gap between client objects.
                    let new_id: u32 = *from_bytes(&bytes[8..12]);
                    state.xdg_toplevel_id = Some(new_id);
                    state.client_blacklist_ids.push(new_id); // we only need to send events to this, not the other way around
                    MessageOperation::Replace(create_useless_object(new_id, state))
                }
                // get_popup
                2 => {}
                _ => MessageOperation::Drop, // Protocol error
            }
        } else if let Some(id) = state.xdg_decoration_id
            && obj_id == id
        {
            match opcode {
                // destroy
                0 => {
                    let mut msg: Vec<u8> = bytes[0..8].iter().cloned().collect();
                    msg[4] = 1;
                    MessageOperation::Replace(msg)
                }
                // get_toplevel_decoration
                1 => {
                    let new_id = *from_bytes(&bytes[8..12]);
                    // state.xdg_toplevel_decoration_id = Some(new_id);
                    debug!("Blacklisting id {}", new_id);
                    state.client_blacklist_ids.push(new_id);
                    MessageOperation::Replace(create_useless_object(new_id, state))
                }
                _ => MessageOperation::Drop, // Protocol error
            }
        } else if let Some(layer_shell_id) = state.layer_shell_id
            && obj_id == layer_shell_id
            && opcode == 2
            && state.layer_surface_id.is_none()
        // TODO: what to do with multiple windows?
        {
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

            // TODO: Destroy layer shell and bind xdg_wm_base?

            // Update state
            debug!("Sending layer surface creation messages.");
            state.layer_surface_id = Some(new_id);

            MessageOperation::Replace(message)
        } else {
            MessageOperation::Keep
        }
    })
}

pub fn scan_events(bytes: &[u8], state: &mut State) -> Vec<u8> {
    scan_messages(bytes, state, |bytes, state, obj_id, opcode| {
        // wl_registry::global
        if let Some(id) = state.registry_id
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
                    debug!("Found zwlr_layer_shell_v1 advertised.");
                    state.advertised_layer_shell_name = Some(name);
                }
            }
        } else if let Some(id) = state.layer_surface_id
            && obj_id == id
        {
            return match opcode {
                // configure
                0 => {
                    let mut new_msgs = Vec::new();
                    // Convert to xdg_surface::configure
                    new_msgs.extend_from_slice(&bytes[0..12]);
                    new_msgs[6] = 12;

                    // Add an extra xdg_toplevel::configure
                    if let Some(toplevel_id) = state.xdg_toplevel_id {
                        new_msgs.extend_from_slice(&toplevel_id.to_le_bytes());
                        new_msgs.extend_from_slice(&0u16.to_le_bytes());
                        new_msgs.extend_from_slice(&20u16.to_le_bytes());
                        new_msgs.extend_from_slice(&bytes[12..16]);
                        new_msgs.extend_from_slice(&bytes[16..20]);
                        new_msgs.extend_from_slice(&0u32.to_le_bytes());
                    }

                    MessageOperation::Replace(new_msgs)
                }
                1 => {
                    if let Some(id) = state.xdg_toplevel_id {
                        let mut new_msg: Vec<u8> = id.to_le_bytes().to_vec();
                        new_msg.extend_from_slice(&bytes[4..8]);
                        MessageOperation::Replace(new_msg)
                    } else {
                        MessageOperation::Drop
                    }
                }
                _ => MessageOperation::Drop, // Protocol error
            };
        }
        MessageOperation::Keep
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
                    "[DROPPED MESSAGE] Object: {}, Opcode: {}, Size: {}\nFull Message: {:?}",
                    obj_id,
                    opcode,
                    size,
                    &bytes[0..usize_size]
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

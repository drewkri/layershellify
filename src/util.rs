use nix::sys::socket::{ControlMessage, ControlMessageOwned};

use crate::State;

/// Length of "zwlr_layer_shell_v1"
pub const LAYER_SHELL_STRING_LEN: u32 = 20;
/// Template used to bind to zwlr_layer_shell_v1. Prefix
/// with the u32 name advertised by the compositor.
pub const LAYER_SHELL_BIND_TEMPLATE: [u8; 28] = [
    /* name must go here (u32) */
    20, 0, 0, 0, // String size
    122, 119, 108, 114, 95, 108, 97, 121, 101, 114, 95, // String + null terminator
    115, 104, 101, 108, 108, 95, 118, 49, 0, // String + null terminator (continued)
    5, 0, 0, 0, // Version
       /* New ID goes here */
];
/// String parameter for "io.github.DrewCodesBadly.Layershellify"
pub const LAYERSHELLIFY_STRING: [u8; 44] = [
    39, 0, 0, 0, // size (16)
    105, 111, 46, 103, 105, 116, 104, 117, 98, 46, 68, 114, 101, 119, 67, 111, 100, 101, 115, 66,
    97, 100, 108, 121, 46, 76, 97, 121, 101, 114, 115, 104, 101, 108, 108, 105, 102, 121, 0, 0,
];
/// "xdg_wm_base" in bytes, with a null terminator
pub const XDG_WM_BASE_STRING: [u8; 12] = [120, 100, 103, 95, 119, 109, 95, 98, 97, 115, 101, 0];
/// "zxdg_decoration_manager_v1" in bytes, with a null terminator, and pad byte
pub const ZXDG_DECORATION_MANAGER_STRING: [u8; 27] = [
    122, 120, 100, 103, 95, 100, 101, 99, 111, 114, 97, 116, 105, 111, 110, 95, 109, 97, 110, 97,
    103, 101, 114, 95, 118, 49, 0,
];

/// """Converts""" a vector of ControlMessageOwned into Vec<ControlMessage>.
/// However since Wayland only bothers with file descriptors any other type
/// of control message is completely ignored.
pub fn to_borrowd_cmsgs<'a>(vec: &'a Vec<ControlMessageOwned>) -> Vec<ControlMessage<'a>> {
    let mut out = Vec::new();

    for msg in vec {
        match msg {
            ControlMessageOwned::ScmRights(items) => out.push(ControlMessage::ScmRights(items)),
            _ => todo!(),
        }
    }

    out
}

pub fn message_header(object_id: u32, opcode: u16, size: u16) -> Vec<u8> {
    let mut vec = Vec::with_capacity(size as usize);
    vec.extend_from_slice(&object_id.to_le_bytes());
    vec.extend_from_slice(&opcode.to_le_bytes());
    vec.extend_from_slice(&size.to_le_bytes());
    vec
}

/// Returns a request which will create a pointless object at object_id, to replace
/// something the client tried to create that won't work.
/// This is currently implemented by binding zwlr_layer_shell_v1 again at the new id.
/// Getting another registry does not work because the server will send globals and confuse the client.
///
/// `state` must have a valid registry id and advertised layer shell id for this to work.
/// This is a convenience function that calls `bind_layer_shell`.
pub fn create_useless_object(object_id: u32, state: &State) -> Vec<u8> {
    bind_layer_shell(
        state.registry_id.unwrap(),
        state.advertised_layer_shell_name.unwrap(),
        object_id,
    )
}

pub fn bind_layer_shell(registry_id: u32, global_name: u32, new_id: u32) -> Vec<u8> {
    let mut message = message_header(
        registry_id,
        0,
        8 + 4 + LAYER_SHELL_BIND_TEMPLATE.len() as u16 + 4,
    );
    message.extend_from_slice(&global_name.to_le_bytes());
    message.extend_from_slice(&LAYER_SHELL_BIND_TEMPLATE);
    message.extend_from_slice(&new_id.to_le_bytes());

    message
}

use nix::sys::socket::{ControlMessage, ControlMessageOwned};

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

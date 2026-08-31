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

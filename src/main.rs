use std::{
    env, fs,
    io::{IoSlice, IoSliceMut},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::Path,
    time::Duration,
};

use calloop::{
    EventLoop, Interest,
    generic::{FdWrapper, Generic},
};
use log::{debug, warn};
use nix::{
    cmsg_space,
    libc::close,
    sys::socket::{ControlMessageOwned, MsgFlags, recvmsg, sendmsg},
};

use crate::{
    decode::{scan_events, scan_requests},
    util::to_borrowd_cmsgs,
};

mod decode;
mod util;

/// Shared state of the proxy
struct State {
    compositor_socket: UnixStream,
    app_socket: UnixStream,

    // Important ids
    registry_id: Option<u32>,
    advertised_layer_shell_name: Option<u32>,
    advertised_xdg_wm_base_name: Option<u32>,
    /// This is the ID we initially bind layer shell to, later it will become
    /// xdg_wm_base again.
    layer_shell_id: Option<u32>,
    layer_surface_id: Option<u32>,
    xdg_toplevel_id: Option<u32>,
    xdg_decoration_id: Option<u32>,
    xdg_toplevel_decoration_id: Option<u32>,
    client_blacklist_ids: Vec<u32>,
    needs_no_csd: bool,
    xdg_surfaces: Vec<u32>,
    // Settings (TODO)
}

fn main() {
    env_logger::init();

    // let logfile = log4rs::append::file::FileAppender::builder()
    //     .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
    //         "{l} - {m}\n",
    //     )))
    //     .build("log.txt")
    //     .unwrap();

    // let config = log4rs::Config::builder()
    //     .appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
    //     .build(
    //         log4rs::config::Root::builder()
    //             .appender("logfile")
    //             .build(log::LevelFilter::Debug),
    //     )
    //     .unwrap();
    // log4rs::init_config(config).unwrap();

    let wayland_socket = env::var("WAYLAND_SOCKET");
    let client_display_name = if let Ok(display_name) = env::var("WAYLAND_DISPLAY") {
        display_name
    } else {
        "wayland-0".to_owned()
    };

    let runtime_dir = env::var("XDG_RUNTIME_DIR").expect("Can't find XDG_RUNTIME_DIR");

    let client_socket_name = if let Ok(name) = wayland_socket {
        name
    } else {
        format!("{}/{}", runtime_dir, client_display_name)
    };
    let client_socket =
        UnixStream::connect(client_socket_name).expect("Cannot connect to wayland compositor.");

    // Unlink the old socket file if it exists - fixes annoying err 98
    let server_socket_path = format!("{}/wayland-0", runtime_dir);
    if Path::new(&server_socket_path).exists() {
        let _ = fs::remove_file(&server_socket_path);
    }

    // TODO: new name other than wayland-0
    let server_socket =
        UnixListener::bind(server_socket_path).expect("Can't bind new wayland socket.");

    // We will not connect to any other clients.
    let (embedded_client, _embedded_client_addr) = server_socket
        .accept()
        .expect("Can't connect to client application.");

    let mut state = State {
        compositor_socket: client_socket,
        app_socket: embedded_client,
        client_blacklist_ids: Vec::new(),
        registry_id: None,
        advertised_layer_shell_name: None,
        advertised_xdg_wm_base_name: None,
        layer_shell_id: None,
        xdg_toplevel_id: None,
        layer_surface_id: None,
        xdg_decoration_id: None,
        xdg_toplevel_decoration_id: None,
        needs_no_csd: true,
        xdg_surfaces: Vec::new(),
    };
    let c_fd = state.compositor_socket.as_raw_fd();
    let a_fd = state.app_socket.as_raw_fd();

    let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    // should be large enough to hold any message easily
    let mut client_read_msg_buffer = [0; 2048];
    // Usually only ever 1 FD but just to be safe I've allocated space for 8.
    // It was 4 but I occasionally saw seemingly random ENOBUFS errors so I upped it to 8.
    let mut client_ancillary_buffer = cmsg_space!([RawFd; 8]);
    let mut server_read_msg_buffer = [0; 2048];
    let mut server_ancillary_buffer = cmsg_space!([RawFd; 8]);
    let handle = event_loop.handle();

    // Handle events coming from the app
    handle
        .insert_source(
            Generic::new(
                // should be safe as it is only polling the fd
                unsafe { FdWrapper::new(a_fd) },
                Interest::READ,
                calloop::Mode::Level,
            ),
            move |_, _, data| {
                // we only listen for readable so the readiness does not need to be checked
                if let Ok(recv_msg) = recvmsg::<()>(
                    data.app_socket.as_raw_fd(),
                    &mut [IoSliceMut::new(&mut client_read_msg_buffer)],
                    Some(&mut client_ancillary_buffer),
                    MsgFlags::MSG_CMSG_CLOEXEC,
                ) {
                    if recv_msg.bytes > 0 {
                        debug!("Reading requests.");
                        let bytes = recv_msg.iovs().next().unwrap();
                        let output_bytes = scan_requests(&bytes[0..recv_msg.bytes], data);
                        debug!("Requests ended.");
                        let owned_msgs = recv_msg.cmsgs().unwrap().collect();
                        let msgs = to_borrowd_cmsgs(&owned_msgs);
                        sendmsg::<()>(
                            data.compositor_socket.as_raw_fd(),
                            &[IoSlice::new(&output_bytes)],
                            &msgs,
                            recv_msg.flags,
                            None,
                        )
                        .unwrap();
                        // Close fds to avoid fd leak
                        for msg in owned_msgs {
                            if let ControlMessageOwned::ScmRights(fds) = msg {
                                for fd in fds {
                                    unsafe { close(fd) };
                                }
                            }
                        }
                    }
                } else {
                    warn!("Read failed")
                }
                Ok(calloop::PostAction::Continue)
            },
        )
        .unwrap();

    // Handle events coming from the compositor
    handle
        .insert_source(
            Generic::new(
                // should be safe as it is only polling the fd
                unsafe { FdWrapper::new(c_fd) },
                Interest::READ,
                calloop::Mode::Level,
            ),
            move |_, _, data| {
                // we only listen for readable so the readiness does not need to be checked
                if let Ok(recv_msg) = recvmsg::<()>(
                    data.compositor_socket.as_raw_fd(),
                    &mut [IoSliceMut::new(&mut server_read_msg_buffer)],
                    Some(&mut server_ancillary_buffer),
                    MsgFlags::MSG_CMSG_CLOEXEC,
                ) {
                    if recv_msg.bytes > 0 {
                        debug!("Reading events.");
                        let bytes = &recv_msg.iovs().next().unwrap();
                        let output_bytes = scan_events(&bytes[0..recv_msg.bytes], data);
                        debug!("Events ended.");
                        let owned_msgs = recv_msg.cmsgs().unwrap().collect();
                        let msgs = to_borrowd_cmsgs(&owned_msgs);
                        sendmsg::<()>(
                            data.app_socket.as_raw_fd(),
                            &[IoSlice::new(&output_bytes)],
                            &msgs,
                            recv_msg.flags,
                            None,
                        )
                        .unwrap();
                        // Close fds to avoid fd leak
                        for msg in owned_msgs {
                            if let ControlMessageOwned::ScmRights(fds) = msg {
                                for fd in fds {
                                    unsafe { close(fd) };
                                }
                            }
                        }
                    }
                } else {
                    warn!("Read failed");
                }
                Ok(calloop::PostAction::Continue)
            },
        )
        .unwrap();

    let _ = event_loop.run(Duration::from_millis(500), &mut state, |_| {
        // timeout callback - unneeded
    });
}

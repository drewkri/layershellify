use std::{
    env, fs,
    io::{IoSlice, IoSliceMut},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::Path,
    process::{Child, Command, exit},
    time::Duration,
};

use calloop::{
    EventLoop, Interest,
    generic::{FdWrapper, Generic},
};
use clap::Parser;
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

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Anchor to the left side. Can be combined with other anchors. At least one anchor must be set.
    #[arg(short = 'L', long, default_value_t = false)]
    anchor_left: bool,
    /// Anchor to the right side. Can be combined with other anchors. At least one anchor must be set.
    #[arg(short = 'R', long, default_value_t = false)]
    anchor_right: bool,
    /// Anchor to the top. Can be combined with other anchors. At least one anchor must be set.
    #[arg(short = 'T', long, default_value_t = false)]
    anchor_top: bool,
    /// Anchor to the bottom. Can be combined with other anchors. At least one anchor must be set.
    #[arg(short = 'B', long, default_value_t = false)]
    anchor_bottom: bool,

    /// Width of the new layer shell window
    #[arg(short = 'W', long, default_value_t = 400)]
    width: u32,
    /// Height of the new layer shell window
    #[arg(short = 'H', long, default_value_t = 400)]
    height: u32,
    /// Allow applications to have client-side decorations. By default they will be forced off.
    #[arg(short, long, default_value_t = false)]
    allow_csd: bool,
    /// Margin to place on all sides of the app
    #[arg(short, long, default_value_t = 0)]
    margins: u32,
    /// Layer shell layer to place the app on. 0 = background, 1 = bottom, 2 = top, 3 = overlay.
    #[arg(short, long, default_value_t = 3)]
    layer: u32,

    /// Command to execute to start the application
    #[arg(last = true)]
    execute_command: Vec<String>,
}

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
    // Settings
    cli: Cli,

    child_process_handle: Child,
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

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
    let mut wayland_socket_idx = 0;
    let server_socket = loop {
        if wayland_socket_idx > 100 {
            panic!("Unable to create wayland socket.")
        }
        let server_socket_path = format!("{}/wayland-{}", runtime_dir, wayland_socket_idx);
        if Path::new(&server_socket_path).exists() {
            let _ = fs::remove_file(&server_socket_path);
        }

        let maybe_socket = UnixListener::bind(server_socket_path);
        if let Ok(socket) = maybe_socket {
            break socket;
        } else {
            wayland_socket_idx += 1;
            continue;
        }
    };
    // Safety: single-threaded program
    unsafe {
        env::set_var(
            "WAYLAND_DISPLAY",
            &format!("wayland-{}", wayland_socket_idx),
        );
    }
    // Run the app specified
    let mut cmd_iter = cli.execute_command.iter();
    let mut cmd = Command::new(cmd_iter.next().expect("No command provided to start app."));
    while let Some(arg) = cmd_iter.next() {
        cmd.arg(arg);
    }
    debug!(
        "Running command: {:?}",
        cmd.get_args().collect::<Vec<&std::ffi::OsStr>>()
    );
    let handle = cmd.spawn().expect("Failed to start application.");

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
        needs_no_csd: !cli.allow_csd,
        xdg_surfaces: Vec::new(),
        child_process_handle: handle,
        cli,
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

    let _ = event_loop.run(Duration::from_millis(500), &mut state, |state| {
        // timeout callback - check if child is still alive
        if let Ok(status) = state.child_process_handle.try_wait() {
            if status.is_some() {
                debug!("Exiting since child process ended.");
                exit(0);
            }
        } else {
            warn!("Error getting child process exit status, exiting.");
            exit(0); // Error?
        }
    });
}

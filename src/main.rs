use std::{
    env,
    fmt::write,
    fs,
    io::{IoSlice, IoSliceMut, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    panic,
    path::Path,
    ptr::read,
    time::{Duration, Instant},
};

use bytemuck::bytes_of;
use calloop::{
    EventLoop, Interest,
    generic::{FdWrapper, Generic},
    io::Writable,
};
use log::{LevelFilter, debug, error, trace, warn};
use log4rs::{append::file::FileAppender, encode::pattern::PatternEncoder};
use nix::{
    cmsg_space,
    libc::{MSG_CTRUNC, SO_RCVBUF, close},
    sys::socket::{
        ControlMessageOwned, GetSockOpt, MsgFlags, SetSockOpt, getsockopt, recv, recvmsg, sendmsg,
        setsockopt,
        sockopt::{self, RcvBuf},
    },
};

use crate::{decode::decode_messages, util::to_borrowd_cmsgs};

mod decode;
mod util;

struct State {
    compositor_socket: UnixStream,
    app_socket: UnixStream,
}

// Storing here for now.
// This error happens because the proxy's socket receive buffer is overflowing before your program can process or forward the incoming data.
// When an application creates a shared memory pool (wl_shm::create_pool), it passes a file descriptor (fd) over the Unix socket via auxiliary control data (SCM_RIGHTS). If the proxy reads the main socket stream without properly handling or extracting the pending file descriptors—or if processing the buffer stalls while waiting to handle fd passing—the incoming kernel socket buffer quickly fills up, triggering ENOBUFS (No buffer space available) on subsequent recvmsg() calls.
// Here are the most common causes in Rust Wayland proxy implementations:
//     Unconsumed Control Data (ancillary buffer): If recvmsg receives file descriptors in the SCM_RIGHTS control message but your code fails to clear or correctly
// size the control buffer (cmsg), the kernel stops delivering data or buffers get clogged. Ensure your buffer supplied to nix::sys::socket::recvmsg has enough space
// for control messages (CmsgSpace).
//     FD Leak / Socket Backpressure: If wl_shm::create_pool is called repeatedly in a tight loop by the client, the client is sending a rapid burst of data.
// If your proxy blocks while trying to forward the file descriptor or message to the main compositor socket, the client-facing socket buffer overflows.
//     Non-blocking Socket Mismatch: If the socket is set to non-blocking mode and you aren't draining the socket entirely until EWOULDBLOCK/EAGAIN, or if the
// compositor side isn't keeping up, socket buffers back up until ENOBUFS is thrown.
//     Socket Buffer Size Limits: The OS default buffer size for the socket might be too small for heavy wl_shm burst traffic. You can inspect or increase
// the socket receive buffer using nix::sys::socket::setsockopt with SO_RCVBUF.

fn main() {
    env_logger::init();

    // let logfile = FileAppender::builder()
    //     .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
    //     .build("log.txt")
    //     .unwrap();

    // let config = log4rs::Config::builder()
    //     .appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
    //     .build(
    //         log4rs::config::Root::builder()
    //             .appender("logfile")
    //             .build(LevelFilter::Debug),
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

    // embedded_client.set_nonblocking(false).unwrap();
    // client_socket.set_nonblocking(false).unwrap();

    let mut state = State {
        compositor_socket: client_socket,
        app_socket: embedded_client,
    };
    let c_fd = state.compositor_socket.as_raw_fd();
    let a_fd = state.app_socket.as_raw_fd();

    let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    // should be large enough to hold any message easily
    let mut client_read_msg_buffer = [0; 2048];
    // 4 FDs should be enough space. hopefully.
    let mut client_ancillary_buffer = cmsg_space!([RawFd; 4]);
    let mut server_read_msg_buffer = [0; 2048];
    let mut server_ancillary_buffer = cmsg_space!([RawFd; 4]);
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
                        let bytes = recv_msg.iovs().next().unwrap();
                        let decoded = decode_messages(&bytes[0..recv_msg.bytes]);
                        for msg in decoded {
                            debug!("[APP] {}", msg);
                        }
                        debug!("[APP] Messages ended.");
                        let owned_msgs = recv_msg.cmsgs().unwrap().collect();
                        let msgs = to_borrowd_cmsgs(&owned_msgs);
                        sendmsg::<()>(
                            data.compositor_socket.as_raw_fd(),
                            &[IoSlice::new(&bytes[0..recv_msg.bytes])],
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
                    warn!("Error receiving message.")
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
                    let bytes = &recv_msg.iovs().next().unwrap();
                    if recv_msg.bytes > 0 {
                        let decoded = decode_messages(&bytes[0..recv_msg.bytes]);
                        for msg in decoded {
                            debug!("[SERVER] {}", msg);
                        }
                        debug!("[SERVER] Messages ended.");
                        let owned_msgs = recv_msg.cmsgs().unwrap().collect();
                        let msgs = to_borrowd_cmsgs(&owned_msgs);
                        sendmsg::<()>(
                            data.app_socket.as_raw_fd(),
                            &[IoSlice::new(&bytes[0..recv_msg.bytes])],
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

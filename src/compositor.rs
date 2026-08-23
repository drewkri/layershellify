/// Largely copied from Smithay's Smallvil example
use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatState},
    reexports::{
        calloop::{EventLoop, Interest, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_server::{
            Display, DisplayHandle,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_shm,
        },
    },
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::State;

mod handlers;
mod input;

pub struct ServerState {
    display_handle: DisplayHandle,
    shm_state: ShmState,
    compositor_state: CompositorState,
    space: Space<Window>,
    popups: PopupManager,
    shell_state: XdgShellState,
    seat_state: SeatState<State>,
}

impl ServerState {
    pub fn new(display: Display<State>, event_loop: &mut EventLoop<State>) -> Self {
        let display_handle = display.handle();
        let socket_name = Self::init_wayland_listener(display, event_loop);
        println!("{:?}", socket_name);
        Self {
            shm_state: ShmState::new::<State>(
                &display_handle,
                [wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888],
            ),
            compositor_state: CompositorState::new::<State>(&display_handle),
            space: Space::default(),
            popups: PopupManager::default(),
            shell_state: XdgShellState::new::<State>(&display_handle),
            seat_state: SeatState::default(),
            display_handle,
        }
    }

    fn init_wayland_listener(
        display: Display<State>,
        event_loop: &mut EventLoop<State>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .server
                    .display_handle
                    .insert_client(client_stream, Arc::new(ServerClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }
}

/// Data associated with a wayland client that connects to the compositor.
/// One instance of this type per client.
/// This name is confusing but I need it to not conflict with the CLIENT end of this crate.
#[derive(Default)]
pub struct ServerClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ServerClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

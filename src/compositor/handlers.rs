use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_output, delegate_shm, delegate_xdg_shell,
    desktop::{PopupKind, Window, find_popup_root_surface, get_popup_toplevel_coords},
    reexports::wayland_server::{
        Client, Dispatch,
        protocol::{wl_shm::WlShm, wl_shm_pool::WlShmPool, wl_surface::WlSurface},
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent,
            is_sync_subsurface, with_states,
        },
        output::OutputHandler,
        shell::xdg::{PopupSurface, XdgShellHandler, XdgToplevelSurfaceData},
        shm::{ShmHandler, ShmPoolUserData},
    },
};
use smithay_client_toolkit::delegate_dispatch2;

use crate::{
    State,
    compositor::{ServerClientState, ServerState},
};

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.server.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ServerClientState>()
            .unwrap()
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        // Updates buffer
        on_commit_buffer_handler::<WlSurface>(surface);
        // Calls on_commit on the appropriate window, copied from Smallvil
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .server
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == &root)
            {
                window.on_commit();
            }
        };

        // Handle initial XdgShell configure. Copied from Smallvil.
        if let Some(window) = self
            .server
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|tl| tl.wl_surface() == surface))
        {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            if !initial_configure_sent {
                window.toplevel().unwrap().send_configure();
            }
        }

        // Handle popup commits. Also copied from Smallvil.
        self.server.popups.commit(surface);
        if let Some(popup) = self.server.popups.find_popup(surface) {
            match popup {
                PopupKind::Xdg(ref xdg) => {
                    if !xdg.is_initial_configure_sent() {
                        // NOTE: This should never fail as the initial configure is always
                        // allowed.
                        xdg.send_configure().expect("initial configure failed");
                    }
                }
                PopupKind::InputMethod(ref _input_method) => {}
            }
        }
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(
        &mut self,
        buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    ) {
        // nothing necessary here
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &smithay::wayland::shm::ShmState {
        &self.server.shm_state
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut smithay::wayland::shell::xdg::XdgShellState {
        &mut self.server.shell_state
    }

    // TODO: Fix to handle more windows in main compositor
    fn new_toplevel(&mut self, surface: smithay::wayland::shell::xdg::ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.server.space.map_element(window, (0, 0), false);
    }

    fn new_popup(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: smithay::wayland::shell::xdg::PositionerState,
    ) {
        self.server.unconstrain_popup(&surface);
        let _ = self.server.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn grab(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        serial: smithay::utils::Serial,
    ) {
        // unneeded
    }

    fn reposition_request(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: smithay::wayland::shell::xdg::PositionerState,
        token: u32,
    ) {
        // do not move windows.
    }
}

impl OutputHandler for State {}

// Copied from smallvil
impl ServerState {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();
        let window_geo = self.space.element_geometry(window).unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

delegate_dispatch2!(State);
delegate_output!(State);
delegate_xdg_shell!(State);
delegate_compositor!(State);
delegate_shm!(State);

use std::num::NonZeroU32;

use smithay::{
    backend::renderer::element::RenderElement,
    desktop::{WindowSurfaceType, space::space_render_elements},
    reexports::wayland_server::Resource,
    wayland::seat::WaylandFocus,
};
// most boilerplate copied from smithay examples
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::client::{
        Connection, EventQueue, QueueHandle,
        globals::registry_queue_init,
        protocol::{wl_output, wl_shm, wl_surface},
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};

pub mod bridge;
use crate::State;

pub struct ClientState {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm_state: Shm,

    pool: SlotPool,

    width: u32,
    height: u32,
    buffer: Option<Buffer>,
    layer: LayerSurface,
    damaged: bool,
    first_configure: bool,
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Not needed
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.client.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.client.shm_state
    }
}

impl State {
    pub fn draw(&mut self, qh: &QueueHandle<State>) {
        println!("Redraw");
        let width = self.client.width;
        let height = self.client.height;
        let stride = self.client.width as i32 * 4;

        let (buffer, canvas) = self
            .client
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("create buffer");

        let elements = space_render_elements(
            &mut self.renderer,
            [self.server.space()],
            self.server.output.as_ref().unwrap(),
            1.0,
        )
        .unwrap();
        // let x = self.server.space()
        for el in elements {
            let storage = el.underlying_storage(&mut self.renderer).unwrap();
        }
        // {
        //     canvas
        //         .chunks_exact_mut(4)
        //         .enumerate()
        //         .for_each(|(index, chunk)| {
        //             let a = 0xFF;
        //             let r = 0;
        //             let g = 0;
        //             let b = 0;
        //             let color: u32 = (a << 24) + (r << 16) + (g << 8) + b;

        //             let array: &mut [u8; 4] = chunk.try_into().unwrap();
        //             *array = color.to_le_bytes();
        //         });
        // }

        // Damage the entire window
        self.client
            .layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);

        // TODO: Stop constantly requesting frames (if possible)
        // Request our next frame
        self.client.layer.wl_surface().frame(
            qh,
            FrameCallbackData(self.client.layer.wl_surface().clone()),
        );

        // Attach and commit to present.
        buffer
            .attach_to(self.client.layer.wl_surface())
            .expect("buffer attach");
        self.client.layer.commit();

        // TODO save and reuse buffer when the window size is unchanged.  This is especially
        // useful if you do damage tracking, since you don't need to redraw the undamaged parts
        // of the canvas.
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.client.registry_state
    }

    registry_handlers!(OutputState);
}

delegate_registry!(State);

impl LayerShellHandler for State {
    fn closed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) {
        // Not needed
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
        configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
        serial: u32,
    ) {
        self.client.width = NonZeroU32::new(configure.new_size.0).map_or(256, NonZeroU32::get);
        self.client.height = NonZeroU32::new(configure.new_size.1).map_or(256, NonZeroU32::get);

        // Initiate the first draw.
        if self.client.first_configure {
            self.client.first_configure = false;
            self.draw(qh);
        }
    }
}

pub fn open_window(layer: Layer, anchor: Anchor) -> (ClientState, EventQueue<State>) {
    let conn = Connection::connect_to_env().unwrap();

    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    let surface = compositor_state.create_surface(&qh);
    let layer_surface =
        layer_shell.create_layer_surface(&qh, surface, layer, Some("layershellify"), None);
    layer_surface.set_anchor(anchor);
    layer_surface.set_size(150, 150);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

    let shm_state = Shm::bind(&globals, &qh).expect("wl_shm not available");

    layer_surface.commit();

    let pool = SlotPool::new(1, &shm_state).expect("Failed to create pool");

    let state = ClientState {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor_state,
        shm_state,

        pool,
        width: 1000,
        height: 1000,
        buffer: None,
        layer: layer_surface,
        damaged: false,
        first_configure: true,
    };

    (state, event_queue)
}

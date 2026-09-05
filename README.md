# Layershellify

A tool to force Wayland windows onto a layer shell surface. Currently only supports Wayland applications
although running in Xwayland rootful mode will likely work.

## Building

Run `cargo build --release`.
Should only compile on Linux since this relies on Linux system calls.

## Usage

Run `layershellify --help` to see options.
At minimum, you need to set at least one anchor flag and then provide a command to run.

## How does it work?

The app creates a proxy wayland display file descriptor which forwards all messages to your main compositor.
It then listens to the messages sent back and forth and alters some of them to make the server think the client is
requesting a zwlr_layer_surface_v1 when it first requests to create an xdg_surface, and then modifies or drops requests
as needed to prevent a resulting protocol error. The server's messages are also altered to appear to be correct configure
requests for what the client thinks is an xdg_surface object.

After the first xdg_surface has been created, any subsequent xdg_surface objects (such as popups or child windows) can
be created at will by the client and the proxy will not make any modifications to the requests aside from altering xdg_popup
objects that are trying to parent to the layer shell surface to avoid a protocol error.

## TODOs (potential crash causes that have yet to be fixed)

1. Actually listen for wl_display::sync requests and the resulting wl_callback::done events to avoid bad message reads, then read all messages at once. This seems to cause issues a lot more often than you would expect so if something is crashing/not working correctly that may be why.
2. Dialog windows may not show up properly, but they don't seem to cause a crash. I'm not sure why this is, it could probably be fixed but it's not a very high priority for me so I haven't bothered to look into it too much yet.

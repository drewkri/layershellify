# Layershellify

A tool to force Wayland windows onto a layer shell surface. Currently only supports Wayland applications
although running in Xwayland rootful mode will likely work.

## Building

Run `cargo build --release`.
Should only compile on Linux since this relies on Linux system calls.

## How does it work?

TODO

## TODOs (potential crash causes that have yet to be fixed)

1. Actually listen for wl_display::sync requests and the resulting wl_callback::done events to avoid bad message reads, then read all messages at once. This seems to cause issues a lot more often than you would expect so if something is crashing/not working correctly that may be why.

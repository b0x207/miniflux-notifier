# Miniflux Notifier

Get desktop notifications at select intervals about new, unread articles from a Miniflux server.

The program is a daemon for linux that can be run on a per-user basis.

## Building

Make sure you have the rust toolchain installed (see [rustup.rs](https://rustup.rs)).

```
cargo build --release
```

The build binary is a single file located at `target/release/miniflux-notifier`.

## Running

The program should be run as a daemon and thus, it is up to you to determine how your system should start it. If you use Systemd, I would recommend using a [user service](https://wiki.archlinux.org/title/Systemd/User).

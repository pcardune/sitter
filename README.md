# Sitter

A simple linux desktop app written in Rust to keep you from sitting at your computer for too long.

This app monitors dbus events from the gnome screen saver to determine when your screen
goes to sleep or wakes up. Upon waking up, you'll be able to use your computer for 30
minutes (or the duration you specify) before a really annoying window shows up that covers
most of your screen.

There is a snooze button to give you 5 extra minutes if necessary.

This app has only been tested with Ubuntu 20, running the gnome desktop.

## Building Debian Package for Ubuntu

0. Install [cargo-deb](https://github.com/kornelski/cargo-deb)

1. Generate app icons (this requires inkscape to be installed)

```sh
./build.sh
```

2. Generate the .deb file

```sh
cargo deb
```

3. Install the .deb file locally:

```sh
sudo dpkg -i target/debian/sitter_0.1.0_amd64.deb
```

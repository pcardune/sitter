# Sitter

A simple desktop app to keep you from sitting at your computer for too long

# Building Debian Package

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

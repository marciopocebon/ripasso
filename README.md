# ripasso
[![Build Status](https://travis-ci.org/cortex/ripasso.svg?branch=master)](https://travis-ci.org/cortex/ripasso)
[![Crates Version](https://meritbadge.herokuapp.com/ripasso)](https://crates.io/crates/ripasso)
[![Documentation Status](https://docs.rs/ripasso/badge.svg)](https://docs.rs/ripasso/)

A simple password manager written in Rust.

The root crate `ripasso` is a library for accessing and decrypting passwords
stored in pass format (GPG-encrypted files), with a file-watcher event emitter.

Multiple UI's in different stages of development are available in subcrates.

To build all UI's:
```
cargo build --all
```

PR's are very welcome!

If you want to talk to the developers, please join our matrix room [here](https://riot.im/app/#/room/#ripasso:matrix.org).

## History
This is a reimplementation of https://github.com/cortex/gopass in Rust. I started it mainly because https://github.com/go-qml/qml
is unmaintained. Also, using a safe language for your passwords seems like a good idea.

## UI's

### Cursive - Terminal interface
![Screenshot of ripasso-cursive](doc/ripasso-cursive.png)

TUI interface based on [cursive](https://github.com/gyscos/Cursive)
Supports password age display and password editing.
I use this as my daily password-manager.

#### Build
```
cargo build -p ripasso-cursive
```


### QT GUI - (unstable)
![Screenshot of ripasso-qt](doc/ripasso-qt.png)
This is mostly working, but needs updates.

#### Build

```
cargo build -p ripasso-qt
```
For it to run, you need to be in the qml directory.

```
cd qml
cargo run
```

### GTK GUI - (WIP)
![Screenshot of ripasso-gtk](doc/ripasso-gtk.png)
Build

```
cargo build -p ripasso-gtk
```


## Build dependencies

### Mac OS X

```
$ brew update
$ brew install automake cmake gettext qt5 gtk+3
$ export PATH="/usr/local/opt/qt/bin:$PATH"
$ git clone https://github.com/cortex/ripasso.git
$ cd ripasso
$ cargo run
```

### Ubuntu
```
$ apt install cargo libgtk-3-dev qtdeclarative5-dev libqt5svg5-dev cmake libncurses-dev libssl-dev
$ cargo build
```

### Arch
```
$ pacman -S qt5-base qt5-svg qt5-declarative
```

## Packaging status

### Arch

TUI version
```
yay install ripasso-cursive
```

### Fedora

Avaliable in [Copr](https://copr.fedorainfracloud.org/coprs/atim/ripasso/)
```
sudo dnf copr enable atim/ripasso -y
```

TUI version
```
sudo dnf install ripasso
```

GTK version (unstable)
```
sudo dnf install ripasso-gtk
```

Qt version (unstable)
```
sudo dnf install ripasso-qt
```

#### Nix

TUI version
```
nix-env -iA nixpkgs.ripasso-cursive
```

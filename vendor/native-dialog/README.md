# native-dialog

[![Crates.io](https://img.shields.io/crates/v/native-dialog.svg)](https://crates.io/crates/native-dialog)
[![Docs.rs](https://docs.rs/native-dialog/badge.svg)](https://docs.rs/native-dialog)
[![License](https://img.shields.io/crates/l/native-dialog.svg)](LICENSE)

A library to display file choosers and message boxes. Supports GNU/Linux, BSD, macOS and Windows.

## Installation

```
cargo add native-dialog
```

## Usage

```rust
use native_dialog::{DialogBuilder, MessageLevel};

let path = DialogBuilder::file()
    .set_location("~/Desktop")
    .add_filter("PNG Image", ["png"])
    .add_filter("JPEG Image", ["jpg", "jpeg"])
    .open_single_file()
    .show()
    .unwrap();

let path = match path {
    Some(path) => path,
    None => return,
};

// Asyncronous Dialog
let yes = DialogBuilder::message()
    .set_level(MessageLevel::Info)
    .set_title("Do you want to open the file?")
    .set_text(format!("{:#?}", path))
    .confirm()
    .spawn()
    .await
    .unwrap();

if yes {
    do_something(path).await;
}
```

## Misc

#### Ugly or blurry dialogs on Windows

Turn on crate features or embed manifests into the `.exe` to enable visual styling and dpi awareness for your program. Check out [examples/windows_manifest](examples/windows_manifest) and [examples/windows_features](examples/windows_features) for example.

#### Linux/BSD dependencies

The implementation for Linux and BSD requires either [Zenity](https://gitlab.gnome.org/GNOME/zenity), [KDialog](https://invent.kde.org/utilities/kdialog), or [YAD](https://github.com/v1cont/yad) being installed; otherwise the `MissingDep` error will be returned.

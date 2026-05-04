# wfd 
[![Build Status](https://github.com/ben-wallis/wfd/workflows/Build/badge.svg)](https://github.com/ben-wallis/wfd/actions)
[![Crates.io](https://img.shields.io/crates/v/wfd)](https://crates.io/crates/wfd)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This crate provides a simple to use abstraction over the Open and Save dialogs in the Windows API, usable under both GNU and MSVC toolchains, with minimal dependencies.

## Examples

### Standard open dialog
```rust
let dialog_result = wfd::open_dialog(Default::default())?;
```

### Folder picker open dialog
```rust
use wfd::{DialogParams};

let params = DialogParams {
    options: FOS_PICKFOLDERS,
    .. Default::default()
};

let dialog_result = wfd::open_dialog(params)?;
```

### Save dialog with custom file extension filters
```rust
use wfd::{DialogParams};

let params = DialogParams {
    title: "Select an image to open",
    file_types: vec![("JPG Files", "*.jpg;*.jpeg"), ("PNG Files", "*.png"), ("Bitmap Files", "*.bmp")],
    default_extension: "jpg",
    ..Default::default()
};

let dialog_result = wfd::save_dialog(params)?;
```

**Further examples can be found in `src\examples`**

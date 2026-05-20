# sanitize-filename

A basic filename sanitizer, based on Node's [sanitize-filename](https://www.npmjs.com/package/sanitize-filename).

Use like:

```rust
extern crate sanitize_filename;

fn main() {
    println!("{}", sanitize_filename::sanitize("some-user-defined/../../../string"));
    // prints some-user-defined......string
}
```

You can also configure a few basic options:

```rust
let options = sanitize_filename::Options {
    truncate: true, // true by default, truncates to 255 bytes
    windows: true, // default value depends on the OS, removes reserved names like `con` from start of strings on Windows
    replacement: "" // str to replace sanitized chars/strings
};

let sanitized = sanitize_filename::sanitize_with_options("some-user-defined/../../../string", options);
```

Also provides a basic command line binary. Use like:

```bash
cargo install sanitize-filename
sanitize-filename my_filename.txt
```

```
Pass a file name to clean to the program (also reads STDIN)

FLAGS:
    -r, --replace <r>          Replacement characters
    --windows, --no-windows    Whether to handle filenames for Windows
    --truncate, --no-truncate  Whether to truncate file names to 255 bytes
```

extern crate wfd;

use wfd::DialogParams;

fn main() {
    let params = DialogParams {
        title: "Select an image to open",
        file_types: vec![("JPG Files", "*.jpg;*.jpeg"), ("PNG Files", "*.png"), ("Bitmap Files", "*.bmp")],
        // Default to PNG Files
        file_type_index: 2,
        // Specifies the default extension before the user changes the File Type dropdown. Note that
        // omitting this field will result in no extension ever being appended to a filename that a
        // user types in even after they change the selected File Type.
        default_extension: "png",
        ..Default::default()
    };

    let result = wfd::open_dialog(params);
    println!("{:?}", result);
}
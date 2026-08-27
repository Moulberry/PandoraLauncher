use fax::decoder;
use fax::tiff::wrap;
use std::fs;
use std::path::Path;

fn convert_fax_to_tiff(input: &Path, width: u16, output: &Path) {
    let data = fs::read(input).expect("Failed to read input file");
    let mut height = 0u32;

    decoder::decode_g4(data.iter().cloned(), width, None, |_transitions| {
        height += 1;
    });

    fs::write(output, wrap(&data, width as _, height)).expect("Failed to write TIFF file");
}

fn parse_filename(name: &str) -> Option<(&str, u16)> {
    let name = name.strip_suffix(".raw")?;
    let (id, rest) = name.split_once('_')?;
    let width_str = rest.strip_prefix("0-w")?;
    let width = width_str.parse().ok()?;
    Some((id, width))
}

#[test]
fn main() {
    let dir = Path::new("test-files/errors");

    for entry in fs::read_dir(dir).expect("Failed to read directory") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();

        if let Some((id, width)) = parse_filename(&name) {
            let tif = dir.join(format!("{}.tif", id));
            if tif.is_file() {
                continue;
            }

            convert_fax_to_tiff(&path, width, &tif);
        }
    }
}

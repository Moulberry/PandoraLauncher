use fax::tiff::wrap;
use fax::{decoder, decoder::pels, BitWriter, Bits, Color, VecWriter};
use std::fs::{self, File};
use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1);
    let input: String = args.next().unwrap();
    let width: u16 = args.next().unwrap().parse().unwrap();
    let output = args.next().unwrap();

    let data = fs::read(&input).unwrap();
    let mut height = 0;
    decoder::decode_g4(data.iter().cloned(), width, None, |transitions| {
        height += 1;
    });

    std::fs::write(output, wrap(&data, width as _, height)).unwrap();
}

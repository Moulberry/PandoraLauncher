fn main() {
    use rgb::prelude::*;
    use rgb::Rgb;

    let px = Rgb {
        r: 255_u8,
        g: 0,
        b: 100,
    };
    #[cfg(feature = "bytemuck")]
    assert_eq!(rgb::bytemuck::cast_slice::<_, u8>(&[px])[0], 255);

    let px = Rgb::<u8>::new(255, 0, 255);
    let inverted: Rgb<u8> = px.map(|ch| 255 - ch);

    println!("{inverted}"); // rgb(0,255,0)
}

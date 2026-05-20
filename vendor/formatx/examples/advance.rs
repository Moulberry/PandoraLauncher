use formatx::Template;

fn main() {
    let mut template = "{percentage color=true:.2} => {percentage color=false:.0}%"
        .parse::<Template>()
        .unwrap();

    template.replace_with_callback("percentage", 99.9999, |fmtval, placeholder| {
        if let Some(color) = placeholder.attr("color") {
            if color == "true" {
                return "\x1b[31m".to_owned() + &fmtval + "\x1b[0m";
            }
        }

        fmtval
    });

    println!("{}", template.text().unwrap());
}

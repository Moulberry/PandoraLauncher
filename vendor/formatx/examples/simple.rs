use formatx::formatx;

fn main() {
    let template = "{} {0:-^10} {percentage:.2}";
    let text = formatx!(template, "world!", "hello", percentage = 99.9999);
    println!("{}", text.unwrap());
}

use formatx::formatx;

fn message(language: &str, name: &str, number: i32) -> String {
    let s = match language {
        "french" => "Bonjour {}, le nombre est {}",
        "spanish" => "Hola {}, el numero es {}",
        _ => "Hi {}, the number is {}",
    };
    formatx!(s, name, number).unwrap()
}

fn main() {
    println!("{}", message("french", "Léa", 1));
    println!("{}", message("spanish", "Sofia", 2));
    println!("{}", message("english", "Ashley", 3));
}

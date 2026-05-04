pub(crate) fn is_number(text: &str) -> bool {
    text.parse::<usize>().is_ok() || text.parse::<isize>().is_ok() || text.parse::<f64>().is_ok()
}

pub(crate) fn is_number_and_positive(text: &str) -> bool {
    if text.parse::<usize>().is_ok() {
        true
    } else if let Ok(num) = text.parse::<isize>() {
        num.is_positive()
    } else if let Ok(num) = text.parse::<f64>() {
        num.is_sign_positive()
    } else {
        false
    }
}

pub(crate) fn usize_token(spec: &str, match_index: usize) -> Option<usize> {
    let mut token = None;
    let mut token_index = match_index + 1;

    while let Ok(token_suffixed) = spec
        .get(match_index..token_index)
        .unwrap_or("null")
        .parse::<usize>()
    {
        token = Some(token_suffixed);
        token_index += 1;
    }

    token
}

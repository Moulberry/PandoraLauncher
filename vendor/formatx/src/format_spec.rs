use crate::error::Error;
use std::fmt::{Debug, Display};

macro_rules! parse {
    ($($arg:tt)*) => {
        return Err($crate::error::Error::new_parse(format!($($arg)*)))
    };
}

macro_rules! ufs {
    ($($arg:tt)*) => {
        return Err($crate::error::Error::new_ufs(format!($($arg)*)))
    };
}

#[derive(Debug, Default, Clone)]
pub struct FormatSpec {
    align: Option<(Option<String>, String)>,
    sign: Option<String>,
    hashtag: bool,
    zero: bool,
    width: Option<usize>,
    precision: Option<usize>,
    types: Option<String>,
}

impl FormatSpec {
    pub(crate) fn new(placeholder: &str) -> Result<Self, Error> {
        if placeholder.rfind(':').is_some() {
            let spec = placeholder.split(':').last().unwrap().to_owned();
            Self::validate(spec.clone(), placeholder)?;
            return Ok(Self::parse(spec));
        }

        Ok(Self::default())
    }

    fn validate(mut spec: String, placeholder: &str) -> Result<(), Error> {
        let mut align_index = match (spec.find('<'), spec.find('^'), spec.find('>')) {
            (None, None, None) => None,
            (Some(left), None, None) => Some(left),
            (None, Some(center), None) => Some(center),
            (None, None, Some(right)) => Some(right),
            _ => parse!("multiple align specifiers used in {{{}}}", placeholder),
        };

        if let Some(x) = align_index {
            if x == 1 {
                spec.replace_range((x - 1)..x, "");
                align_index = Some(0);
            }
        }

        let sign_index = match (spec.find('+'), spec.find('-')) {
            (None, None) => None,
            (Some(positive), None) => Some(positive),
            (None, Some(negative)) => Some(negative),
            _ => parse!("multiple signs used in {{{}}}", placeholder),
        };

        let hashtag_index = spec.find('#');
        let mut zero_index = spec.find('0');
        let precision_index = spec.find('.');

        if let Some(x) = zero_index {
            if let Some(y) = precision_index {
                if x > y {
                    zero_index = None;
                }
            }
        }

        let question_index = spec.find('?');

        if let Some(x) = align_index {
            for (spec_name, spec_index) in [
                ("sign", sign_index),
                ("#", hashtag_index),
                ("0 padding and width", zero_index),
                ("precision", precision_index),
                ("?", question_index),
            ]
            .iter()
            {
                if let Some(spec_index) = spec_index {
                    if x > *spec_index {
                        parse!(
                            "align should be used before {} in {{{}}}",
                            spec_name,
                            placeholder
                        );
                    }
                }
            }
        }

        if let Some(x) = sign_index {
            for (spec_name, spec_index) in [
                ("#", hashtag_index),
                ("0 padding and width", zero_index),
                ("precision", precision_index),
                ("?", question_index),
            ]
            .iter()
            {
                if let Some(spec_index) = spec_index {
                    if x > *spec_index {
                        parse!(
                            "sign should be used before {} in {{{}}}",
                            spec_name,
                            placeholder
                        );
                    }
                }
            }
        }

        if let Some(x) = hashtag_index {
            for (spec_name, spec_index) in [
                ("0 padding and width", zero_index),
                ("precision", precision_index),
                ("?", question_index),
            ]
            .iter()
            {
                if let Some(spec_index) = spec_index {
                    if x > *spec_index {
                        parse!(
                            "# should be used before {} in {{{}}}",
                            spec_name,
                            placeholder
                        );
                    }
                }
            }
        }

        if let Some(x) = zero_index {
            for (spec_name, spec_index) in
                [("precision", precision_index), ("?", question_index)].iter()
            {
                if let Some(spec_index) = spec_index {
                    if x > *spec_index {
                        parse!(
                            "0 should be used before {} in {{{}}}",
                            spec_name,
                            placeholder
                        );
                    }
                }
            }
        }

        if let Some(x) = precision_index {
            for (spec_name, spec_index) in [("?", question_index)].iter() {
                if let Some(spec_index) = spec_index {
                    if x > *spec_index {
                        parse!(
                            "precision should be used before {} in {{{}}}",
                            spec_name,
                            placeholder
                        );
                    }
                }
            }

            if let Some(y) = spec.get((x + 1)..(x + 2)) {
                if y.parse::<usize>().is_err() {
                    parse!(
                        "precision value is not a valid usize in {{{}}}",
                        placeholder
                    );
                }
            } else {
                parse!("precision value not supplied in {{{}}}", placeholder);
            }
        }

        // UNSUPPORTED FORMAT SPECS

        if spec.contains('$') {
            ufs!(
                "parameter setting through $ sign argument is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains(".*") {
            ufs!(
                "asterisk .* formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('o') {
            ufs!(
                "o formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('x') {
            ufs!(
                "x formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('X') {
            ufs!(
                "X formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('p') {
            ufs!(
                "p formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('b') {
            ufs!(
                "b formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('e') {
            ufs!(
                "e formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        if spec.contains('E') {
            ufs!(
                "E formatting is not supported but used in {{{}}}",
                placeholder
            );
        }

        Ok(())
    }

    fn parse(mut spec: String) -> Self {
        let spec_copy = spec.clone();
        let align = if let Some(align) = spec_copy.get(0..1) {
            if align == "<" || align == "^" || align == ">" {
                spec.replace_range(0..1, "");
                Some((None, align.to_owned()))
            } else if let Some(align) = spec_copy.get(1..2) {
                if align == "<" || align == "^" || align == ">" {
                    spec.replace_range(0..2, "");
                    Some((
                        Some(spec_copy.get(0..1).unwrap().to_owned()),
                        align.to_owned(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let spec_copy = spec.clone();
        let sign = if let Some(sign) = spec_copy.get(0..1) {
            if sign == "+" || sign == "-" {
                spec.replace_range(0..1, "");
                Some(sign.to_owned())
            } else {
                None
            }
        } else {
            None
        };

        let hashtag = if let Some(hastag) = spec.get(0..1) {
            if hastag == "#" {
                spec.replace_range(0..1, "");
                true
            } else {
                false
            }
        } else {
            false
        };

        let zero = if let Some(zero) = spec.get(0..1) {
            if zero == "0" {
                spec.replace_range(0..1, "");
                true
            } else {
                false
            }
        } else {
            false
        };

        let width = crate::utils::usize_token(&spec, 0);

        if let Some(x) = width {
            spec = spec.trim_start_matches(&x.to_string()).to_owned();
        }

        let precision = if let Some(point) = spec.get(0..1) {
            if point == "." {
                crate::utils::usize_token(&spec, 1)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(x) = precision {
            spec = spec.trim_start_matches(&format!(".{}", x)).to_owned();
        }

        let types = if spec.is_empty() {
            None
        } else {
            Some(spec.split(' ').next().unwrap().to_owned())
        };

        Self {
            align,
            sign,
            hashtag,
            zero,
            width,
            precision,
            types,
        }
    }

    pub fn format<T: Display + Debug>(&self, value: T) -> String {
        let mut fmtval = format!("{}", value);

        if let Some(precision) = self.precision {
            if self.zero && self.hashtag {
                fmtval = format!("{:#0.1$}", value, precision);
            } else if self.hashtag {
                fmtval = format!("{:#.1$}", value, precision);
            } else {
                fmtval = format!("{:.1$}", value, precision);
            }
        }

        if let Some(types) = &self.types {
            if self.hashtag {
                if types == "?" {
                    return format!("{:#?}", value);
                } else if types == "x?" {
                    return format!("{:#x?}", value);
                } else if types == "X?" {
                    return format!("{:#X?}", value);
                }
            } else if types == "?" {
                return format!("{:?}", value);
            } else if types == "x?" {
                return format!("{:x?}", value);
            } else if types == "X?" {
                return format!("{:X?}", value);
            }
        }

        if let Some(sign) = &self.sign {
            if sign == "+" && !self.zero && crate::utils::is_number_and_positive(&fmtval) {
                fmtval = "+".to_owned() + &fmtval;
            }
        }

        match &self.align {
            Some((Some(fill), align)) => {
                fmtval = fmtval.trim().to_owned();
                let chars_count = fmtval.chars().count();
                let width = self.width.unwrap_or(0);

                if width > chars_count {
                    if align == "<" {
                        fmtval = fmtval + &fill.repeat(width - chars_count);
                    } else if align == "^" {
                        let factor = usize::from(chars_count % 2 != 0);
                        let start = fill.repeat((width - chars_count - factor) / 2);
                        let end = fill.repeat((width - chars_count + factor) / 2);
                        fmtval = start + &fmtval + &end;
                    } else if align == ">" {
                        fmtval = fill.repeat(width - chars_count) + &fmtval;
                    }
                }
            }
            Some((None, align)) => {
                if align == "<" {
                    fmtval = format!("{:<1$}", fmtval, self.width.unwrap_or(0));
                } else if align == "^" {
                    fmtval = format!("{:^1$}", fmtval, self.width.unwrap_or(0));
                } else if align == ">" {
                    fmtval = format!("{:>1$}", fmtval, self.width.unwrap_or(0));
                }
            }
            None => (),
        }

        if let Some(width) = self.width {
            if crate::utils::is_number(&fmtval) {
                let chars_count = fmtval.chars().count();

                if self.zero && width > chars_count {
                    if let Some(sign) = &self.sign {
                        if sign == "+" {
                            if crate::utils::is_number_and_positive(&fmtval) {
                                fmtval =
                                    "+".to_owned() + &"0".repeat(width - chars_count - 1) + &fmtval;
                            } else {
                                fmtval = "-".to_owned()
                                    + &"0".repeat(width - chars_count)
                                    + &fmtval[1..];
                            }
                        } else if sign == "-" {
                            fmtval =
                                "-".to_owned() + &"0".repeat(width - chars_count) + &fmtval[1..];
                        }
                    } else if fmtval.starts_with('-') {
                        fmtval = "-".to_owned() + &"0".repeat(width - chars_count) + &fmtval[1..];
                    } else {
                        fmtval = "0".repeat(width - chars_count) + &fmtval;
                    }
                } else if width > chars_count {
                    fmtval = " ".repeat(width - chars_count) + &fmtval;
                }
            } else if self.zero && self.hashtag {
                fmtval = format!("{:#01$}", fmtval, width);
            } else if self.zero {
                fmtval = format!("{:01$}", fmtval, width);
            } else if self.hashtag {
                fmtval = format!("{:#1$}", fmtval, width);
            } else {
                fmtval = format!("{:1$}", fmtval, width);
            }
        }

        fmtval
    }
}

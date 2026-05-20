use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::str::FromStr;

use crate::error::Error;
use crate::placeholder::Placeholder;

/// String template with [std::fmt](std::fmt) syntax.
///
/// # Example
///
/// ```
/// use formatx::Template;
///
/// let mut template = "{percentage:.2}".parse::<Template>().unwrap();
/// template.replace("percentage", 67.7892);
/// assert_eq!(template.text().unwrap(), "67.79");
/// ```
#[derive(Debug, Clone)]
pub struct Template {
    template: String,
    position: usize,
    placeholders: HashMap<String, Vec<Placeholder>>,
}

impl Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.unchecked_text())
    }
}

impl FromStr for Template {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Template {
    /// Create a new instance of `format_template::Template`.
    ///
    /// # Example
    ///
    /// ```
    /// use formatx::Template;
    ///
    /// let template = Template::new("{percentage:.2}").unwrap();
    /// ```
    pub fn new<T: Into<String>>(template: T) -> Result<Self, Error> {
        let mut template_struct = Self {
            template: {
                // Escaping curly braces
                let mut template = template.into().replace("{{", "[curly=open]");

                while let Some(curly_close) = template.rfind("}}") {
                    template.replace_range(curly_close..=(curly_close + 1), "[curly=close]");
                }

                template
            },
            position: 0,
            placeholders: HashMap::new(),
        };

        // Checking number curly braces
        let curly_open_count = template_struct
            .template
            .chars()
            .filter(|x| x.to_string() == "{")
            .count();
        let curly_close_count = template_struct
            .template
            .chars()
            .filter(|x| x.to_string() == "}")
            .count();

        if curly_open_count != curly_close_count {
            return Err(Error::new_parse(format!(
                "number of opening and closing curly braces are not equal i.e. {} != {}",
                curly_open_count, curly_close_count
            )));
        }

        // Adding position for positional placeholders.
        let mut position = 0;
        let placeholders = template_struct.get_placeholders();

        while template_struct.template.contains("{}") || template_struct.template.contains("{:") {
            while placeholders.contains(&position.to_string()) {
                position += 1;
            }

            match (
                template_struct.template.find("{}"),
                template_struct.template.find("{:"),
            ) {
                (Some(_), None) => {
                    template_struct.template =
                        template_struct
                            .template
                            .replacen("{}", &format!("{{{}}}", position), 1);
                    position += 1;
                }
                (None, Some(_)) => {
                    template_struct.template =
                        template_struct
                            .template
                            .replacen("{:", &format!("{{{}:", position), 1);
                    position += 1;
                }
                (Some(x), Some(y)) => {
                    if y > x {
                        template_struct.template = template_struct.template.replacen(
                            "{}",
                            &format!("{{{}}}", position),
                            1,
                        );
                    } else {
                        template_struct.template =
                            template_struct
                                .template
                                .replacen("{:", &format!("{{{}:", position), 1);
                    }

                    position += 1;
                }
                (None, None) => (),
            }
        }

        // Parsing placeholders
        let mut query_template = template_struct.template.clone();

        for name in template_struct.get_placeholders() {
            let placeholder = Placeholder::new(&query_template, &name);
            let placeholder = placeholder?.unwrap();
            query_template = query_template.replacen(&placeholder.replacer, "", 1);

            if let Some(children) = template_struct.placeholders.get_mut(&name) {
                children.push(placeholder);
            } else {
                template_struct
                    .placeholders
                    .insert(name.to_owned(), vec![placeholder]);
            }
        }

        Ok(template_struct)
    }

    /// Returns list of template placeholders names.
    fn get_placeholders(&self) -> Vec<String> {
        let mut placeholders = vec![];
        let mut template = self.template.clone();

        while template.contains('{') && template.contains('}') {
            if let (Some(start), Some(end)) = (template.find('{'), template.find('}')) {
                let placeholder = template[(start + 1)..end].to_owned();
                template.replace_range(start..=end, "");

                if let Some(colon) = placeholder.find(':') {
                    placeholders.push(placeholder[..colon].split(' ').next().unwrap().to_owned());
                } else {
                    placeholders.push(placeholder.split(' ').next().unwrap().to_owned());
                }
            }
        }

        placeholders
    }

    /// Checks wheter template contains placeholder or not.
    pub fn contains<T: ToString>(&self, placeholder: T) -> bool {
        self.placeholders.contains_key(&placeholder.to_string())
    }

    /// Returns unchecked formatted template.
    /// For `Result` version of fomatted template use `.text` method.
    pub fn unchecked_text(&self) -> String {
        self.template
            .replace("[curly=open]", "{")
            .replace("[curly=close]", "}")
    }

    /// Checks wheter all placeholders are replaced or not and returns formatted template.
    pub fn text(&self) -> Result<String, Error> {
        if self.placeholders.is_empty() {
            Ok(self.unchecked_text())
        } else {
            Err(Error::new_values(format!(
                "missing placeholders values for: {}",
                self.placeholders
                    .keys()
                    .map(|x| {
                        if x.parse::<usize>().is_ok() {
                            format!("{} (positional)", x)
                        } else {
                            format!("{} (named)", x)
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            )))
        }
    }

    /// Replace a template placeholder with a value.
    /// This change is inplace.
    pub fn replace<T, U>(&mut self, placeholder: T, value: U)
    where
        T: ToString,
        U: Display + Debug,
    {
        self.replace_with_callback(placeholder, value, |formatted_value, _| formatted_value)
    }

    /// Replace a template positional placeholder with a value.
    /// This change is inplace.
    pub fn replace_positional<T>(&mut self, value: T)
    where
        T: Display + Debug,
    {
        self.replace_with_callback(self.position.to_string(), value, |formatted_value, _| {
            formatted_value
        });
        self.position += 1;
    }

    /// Replace a template placeholder from a callback `String`.
    /// This change is inplace.
    pub fn replace_from_callback<T, U>(&mut self, placeholder: T, callback: U)
    where
        T: ToString,
        U: Fn(&Placeholder) -> String,
    {
        let placeholder = placeholder.to_string();

        if let Some(placeholders) = self.placeholders.get(&placeholder) {
            for holder in placeholders {
                self.template = self.template.replace(&holder.replacer, &callback(holder));
            }

            let _ = self.placeholders.remove(&placeholder);
        }
    }

    /// Replace a template placeholder with a value and do additional formatting using callback.
    /// This change is inplace.
    pub fn replace_with_callback<T, U, V>(&mut self, placeholder: T, value: U, callback: V)
    where
        T: ToString,
        U: Display + Debug,
        V: Fn(String, &Placeholder) -> String,
    {
        let placeholder = placeholder.to_string();

        if let Some(placeholders) = self.placeholders.get(&placeholder) {
            for holder in placeholders {
                self.template = self.template.replace(
                    &holder.replacer,
                    &callback(holder.format_spec.format(&value), holder),
                );
            }

            let _ = self.placeholders.remove(&placeholder);
        }
    }
}

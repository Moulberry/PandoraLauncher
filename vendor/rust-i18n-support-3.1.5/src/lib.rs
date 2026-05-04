use normpath::PathExt;
use std::fs::File;
use std::io::prelude::*;
use std::{collections::BTreeMap, path::Path};

mod atomic_str;
mod backend;
mod config;
mod cow_str;
mod minify_key;
pub use atomic_str::AtomicStr;
pub use backend::{Backend, BackendExt, SimpleBackend};
pub use config::I18nConfig;
pub use cow_str::CowStr;
pub use minify_key::{
    minify_key, MinifyKey, DEFAULT_MINIFY_KEY, DEFAULT_MINIFY_KEY_LEN, DEFAULT_MINIFY_KEY_PREFIX,
    DEFAULT_MINIFY_KEY_THRESH,
};

type Locale = String;
type Value = serde_json::Value;
type Translations = BTreeMap<Locale, Value>;

pub fn is_debug() -> bool {
    std::env::var("RUST_I18N_DEBUG").unwrap_or_else(|_| "0".to_string()) == "1"
}

/// Merge JSON Values, merge b into a
fn merge_value(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge_value(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (a, b) => {
            *a = b.clone();
        }
    }
}

// Load locales into flatten key, value HashMap
pub fn load_locales<F: Fn(&str) -> bool>(
    locales_path: &str,
    ignore_if: F,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut result: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut translations = BTreeMap::new();
    let locales_path = match Path::new(locales_path).normalize() {
        Ok(p) => p,
        Err(e) => {
            if is_debug() {
                println!("cargo:i18n-error={}", e);
            }
            return result;
        }
    };
    let locales_path = match locales_path.as_path().to_str() {
        Some(p) => p,
        None => {
            if is_debug() {
                println!("cargo:i18n-error=could not convert path");
            }
            return result;
        }
    };

    let path_pattern = format!("{locales_path}/**/*.{{yml,yaml,json,toml}}");

    if is_debug() {
        println!("cargo:i18n-locale={}", &path_pattern);
    }

    // check dir exists
    if !Path::new(locales_path).exists() {
        if is_debug() {
            println!("cargo:i18n-error=path not exists: {}", locales_path);
        }
        return result;
    }

    for entry in globwalk::glob(&path_pattern).expect("Failed to read glob pattern") {
        let entry = entry.unwrap().into_path();
        if is_debug() {
            println!("cargo:i18n-load={}", &entry.display());
        }

        if ignore_if(&entry.display().to_string()) {
            continue;
        }

        let locale = entry
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('.').last())
            .unwrap();

        let ext = entry.extension().and_then(|s| s.to_str()).unwrap();

        let file = File::open(&entry).expect("Failed to open file");
        let mut reader = std::io::BufReader::new(file);
        let mut content = String::new();

        reader
            .read_to_string(&mut content)
            .expect("Read file failed.");

        let trs = parse_file(&content, ext, locale)
            .unwrap_or_else(|_| panic!("Parse file `{}` failed", entry.display()));

        trs.into_iter().for_each(|(k, new_value)| {
            translations
                .entry(k)
                .and_modify(|old_value| merge_value(old_value, &new_value))
                .or_insert(new_value);
        });
    }

    translations.iter().for_each(|(locale, trs)| {
        result.insert(locale.to_string(), flatten_keys("", trs));
    });

    result
}

// Parse Translations from file to support multiple formats
fn parse_file(content: &str, ext: &str, locale: &str) -> Result<Translations, String> {
    let result = match ext {
        "yml" | "yaml" => serde_yaml::from_str::<serde_json::Value>(content)
            .map_err(|err| format!("Invalid YAML format, {}", err)),
        "json" => serde_json::from_str::<serde_json::Value>(content)
            .map_err(|err| format!("Invalid JSON format, {}", err)),
        "toml" => toml::from_str::<serde_json::Value>(content)
            .map_err(|err| format!("Invalid TOML format, {}", err)),
        _ => Err("Invalid file extension".into()),
    };

    match result {
        Ok(v) => match get_version(&v) {
            2 => {
                if let Some(trs) = parse_file_v2("", &v) {
                    return Ok(trs);
                }

                Err("Invalid locale file format, please check the version field".into())
            }
            _ => Ok(parse_file_v1(locale, &v)),
        },
        Err(e) => Err(e),
    }
}

/// Locale file format v1
///
/// For example:
/// ```yml
/// welcome: Welcome
/// foo: Foo bar
/// ```
fn parse_file_v1(locale: &str, data: &serde_json::Value) -> Translations {
    Translations::from([(locale.to_string(), data.clone())])
}

/// Locale file format v2
/// Iter all nested keys, if the value is not a object (Map<locale, string>), then convert into multiple locale translations
///
/// If the final value is Map<locale, string>, then convert them and insert into trs
///
/// For example (only support 1 level):
///
/// ```yml
/// _version: 2
/// welcome.first:
///   en: Welcome
///   zh-CN: 欢迎
/// welcome1:
///   en: Welcome 1
///   zh-CN: 欢迎 1
/// ```
///
/// into
///
/// ```yml
/// en.welcome.first: Welcome
/// zh-CN.welcome.first: 欢迎
/// en.welcome1: Welcome 1
/// zh-CN.welcome1: 欢迎 1
/// ```
fn parse_file_v2(key_prefix: &str, data: &serde_json::Value) -> Option<Translations> {
    let mut trs = Translations::new();

    if let serde_json::Value::Object(messages) = data {
        for (key, value) in messages {
            if let serde_json::Value::Object(sub_messages) = value {
                // If all values are string, then convert them into multiple locale translations
                for (locale, text) in sub_messages {
                    // Ignore if the locale is not a locale
                    // e.g:
                    //  en: Welcome
                    //  zh-CN: 欢迎
                    if text.is_string() {
                        let key = format_keys(&[key_prefix, key]);
                        let sub_trs = BTreeMap::from([(key, text.clone())]);
                        let sub_value = serde_json::to_value(&sub_trs).unwrap();

                        trs.entry(locale.clone())
                            .and_modify(|old_value| merge_value(old_value, &sub_value))
                            .or_insert(sub_value);
                        continue;
                    }

                    if text.is_object() {
                        // Parse the nested keys
                        // If the value is object (Map<locale, string>), iter them and convert them and insert into trs
                        let key = format_keys(&[key_prefix, key]);
                        if let Some(sub_trs) = parse_file_v2(&key, value) {
                            // Merge the sub_trs into trs
                            for (locale, sub_value) in sub_trs {
                                trs.entry(locale)
                                    .and_modify(|old_value| merge_value(old_value, &sub_value))
                                    .or_insert(sub_value);
                            }
                        }
                    }
                }
            }
        }
    }

    if !trs.is_empty() {
        return Some(trs);
    }

    None
}

/// Get `_version` from JSON root
/// If `_version` is not found, then return 1 as default.
fn get_version(data: &serde_json::Value) -> usize {
    if let Some(version) = data.get("_version") {
        return version.as_u64().unwrap_or(1) as usize;
    }

    1
}

/// Join the keys with dot, if any key is empty, omit it.
fn format_keys(keys: &[&str]) -> String {
    keys.iter()
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect::<Vec<String>>()
        .join(".")
}

fn flatten_keys(prefix: &str, trs: &Value) -> BTreeMap<String, String> {
    let mut v = BTreeMap::<String, String>::new();
    let prefix = prefix.to_string();

    match &trs {
        serde_json::Value::String(s) => {
            v.insert(prefix, s.to_string());
        }
        serde_json::Value::Object(o) => {
            for (k, vv) in o {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                v.extend(flatten_keys(key.as_str(), vv));
            }
        }
        serde_json::Value::Null => {
            v.insert(prefix, "".into());
        }
        serde_json::Value::Bool(s) => {
            v.insert(prefix, format!("{}", s));
        }
        serde_json::Value::Number(s) => {
            v.insert(prefix, format!("{}", s));
        }
        serde_json::Value::Array(_) => {
            v.insert(prefix, "".into());
        }
    }

    v
}

#[cfg(test)]
mod tests {
    use super::{merge_value, parse_file};

    #[test]
    fn test_merge_value() {
        let a = serde_json::from_str::<serde_json::Value>(
            r#"{"foo": "Foo", "dar": { "a": "1", "b": "2" }}"#,
        )
        .unwrap();
        let b = serde_json::from_str::<serde_json::Value>(
            r#"{"foo": "Foo1", "bar": "Bar", "dar": { "b": "21" }}"#,
        )
        .unwrap();

        let mut c = a;
        merge_value(&mut c, &b);

        assert_eq!(c["foo"], "Foo1");
        assert_eq!(c["bar"], "Bar");
        assert_eq!(c["dar"]["a"], "1");
        assert_eq!(c["dar"]["b"], "21");
    }

    #[test]
    fn test_parse_file_in_yaml() {
        let content = "foo: Foo\nbar: Bar";
        let mut trs = parse_file(content, "yml", "en").expect("Should ok");
        assert_eq!(trs["en"]["foo"], "Foo");
        assert_eq!(trs["en"]["bar"], "Bar");

        trs = parse_file(content, "yaml", "en").expect("Should ok");
        assert_eq!(trs["en"]["foo"], "Foo");

        trs = parse_file(content, "yml", "zh-CN").expect("Should ok");
        assert_eq!(trs["zh-CN"]["foo"], "Foo");

        parse_file(content, "foo", "en").expect_err("Should error");
    }

    #[test]
    fn test_parse_file_in_json() {
        let content = r#"
        {
            "foo": "Foo",
            "bar": "Bar"
        }
        "#;
        let trs = parse_file(content, "json", "en").expect("Should ok");
        assert_eq!(trs["en"]["foo"], "Foo");
        assert_eq!(trs["en"]["bar"], "Bar");
    }

    #[test]
    fn test_parse_file_in_toml() {
        let content = r#"
        foo = "Foo"
        bar = "Bar"
        "#;
        let trs = parse_file(content, "toml", "en").expect("Should ok");
        assert_eq!(trs["en"]["foo"], "Foo");
        assert_eq!(trs["en"]["bar"], "Bar");
    }

    #[test]
    fn test_get_version() {
        let json = serde_yaml::from_str::<serde_json::Value>("_version: 2").unwrap();
        assert_eq!(super::get_version(&json), 2);

        let json = serde_yaml::from_str::<serde_json::Value>("_version: 1").unwrap();
        assert_eq!(super::get_version(&json), 1);

        // Default fallback to 1
        let json = serde_yaml::from_str::<serde_json::Value>("foo: Foo").unwrap();
        assert_eq!(super::get_version(&json), 1);
    }

    #[test]
    fn test_parse_file_in_json_with_nested_locale_texts() {
        let content = r#"{
            "_version": 2,
            "welcome": {
                "en": "Welcome",
                "zh-CN": "欢迎",
                "zh-HK": "歡迎"
            }
        }"#;

        let trs = parse_file(content, "json", "filename").expect("Should ok");
        assert_eq!(trs["en"]["welcome"], "Welcome");
        assert_eq!(trs["zh-CN"]["welcome"], "欢迎");
        assert_eq!(trs["zh-HK"]["welcome"], "歡迎");
    }

    #[test]
    fn test_parse_file_in_yaml_with_nested_locale_texts() {
        let content = r#"
        _version: 2
        welcome:
            en: Welcome
            zh-CN: 欢迎
            jp: ようこそ
        welcome.sub:
            en: Welcome 1
            zh-CN: 欢迎 1
            jp: ようこそ 1
        "#;

        let trs = parse_file(content, "yml", "filename").expect("Should ok");
        assert_eq!(trs["en"]["welcome"], "Welcome");
        assert_eq!(trs["zh-CN"]["welcome"], "欢迎");
        assert_eq!(trs["jp"]["welcome"], "ようこそ");
        assert_eq!(trs["en"]["welcome.sub"], "Welcome 1");
        assert_eq!(trs["zh-CN"]["welcome.sub"], "欢迎 1");
        assert_eq!(trs["jp"]["welcome.sub"], "ようこそ 1");
    }
}

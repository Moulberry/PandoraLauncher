//! This crate defines `struct`s that can be deserialized with Serde
//! to load and inspect `Cargo.toml` metadata.
//!
//! See `Manifest::from_slice`.

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct I18nConfig {
    #[serde(default = "default_locale")]
    pub default_locale: String,
    #[serde(default = "available_locales")]
    pub available_locales: Vec<String>,
    #[serde(default = "load_path")]
    pub load_path: String,
    #[serde(default = "fallback")]
    pub fallback: Vec<String>,
    #[serde(default = "minify_key")]
    pub minify_key: bool,
    #[serde(default = "minify_key_len")]
    pub minify_key_len: usize,
    #[serde(default = "minify_key_prefix")]
    pub minify_key_prefix: String,
    #[serde(default = "minify_key_thresh")]
    pub minify_key_thresh: usize,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            default_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            load_path: "./locales".to_string(),
            fallback: vec![],
            minify_key: crate::DEFAULT_MINIFY_KEY,
            minify_key_len: crate::DEFAULT_MINIFY_KEY_LEN,
            minify_key_prefix: crate::DEFAULT_MINIFY_KEY_PREFIX.to_string(),
            minify_key_thresh: crate::DEFAULT_MINIFY_KEY_THRESH,
        }
    }
}

impl I18nConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(cargo_root: &Path) -> io::Result<Self> {
        let cargo_file = cargo_root.join("Cargo.toml");
        let mut file = fs::File::open(&cargo_file)
            .unwrap_or_else(|e| panic!("Fail to open {}, {}", cargo_file.display(), e));

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Self::parse(&contents)
    }

    pub fn parse(contents: &str) -> io::Result<Self> {
        let package_metadata = contents.contains("[package.metadata.i18n]");
        let workspace_metadata = contents.contains("[workspace.metadata.i18n]");

        if !contents.contains("[i18n]") && !package_metadata && !workspace_metadata {
            return Ok(I18nConfig::default());
        }

        let contents = if package_metadata {
            contents.replace("[package.metadata.i18n]", "[i18n]")
        } else if workspace_metadata {
            contents.replace("[workspace.metadata.i18n]", "[i18n]")
        } else {
            contents.to_string()
        };

        let mut config: MainConfig = toml::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        // Push default_locale
        config
            .i18n
            .available_locales
            .insert(0, config.i18n.default_locale.clone());

        // unqiue
        config.i18n.available_locales =
            config.i18n.available_locales.into_iter().unique().collect();

        Ok(config.i18n)
    }
}

fn default_locale() -> String {
    I18nConfig::default().default_locale
}

fn available_locales() -> Vec<String> {
    I18nConfig::default().available_locales
}

fn load_path() -> String {
    I18nConfig::default().load_path
}

fn fallback() -> Vec<String> {
    I18nConfig::default().fallback
}

fn minify_key() -> bool {
    I18nConfig::default().minify_key
}

fn minify_key_len() -> usize {
    I18nConfig::default().minify_key_len
}

fn minify_key_prefix() -> String {
    I18nConfig::default().minify_key_prefix
}

fn minify_key_thresh() -> usize {
    I18nConfig::default().minify_key_thresh
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct MainConfig {
    pub i18n: I18nConfig,
}

#[test]
fn test_parse() {
    let contents = r#"
        [i18n]
        default-locale = "en"
        available-locales = ["zh-CN"]
        load-path = "./my-locales"
        fallback = ["zh"]
        minify-key = true
        minify-key-len = 12
        minify-key-prefix = "T_"
        minify-key-thresh = 16
    "#;

    let cfg = I18nConfig::parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
    assert_eq!(cfg.load_path, "./my-locales");
    assert_eq!(cfg.fallback, vec!["zh"]);
    assert!(cfg.minify_key);
    assert_eq!(cfg.minify_key_len, 12);
    assert_eq!(cfg.minify_key_prefix, "T_");
    assert_eq!(cfg.minify_key_thresh, 16);

    let contents = r#"
        [i18n]
        available-locales = ["zh-CN", "de", "de"]
        load-path = "./my-locales"
    "#;
    let cfg = I18nConfig::parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN", "de"]);
    assert_eq!(cfg.load_path, "./my-locales");

    let contents = "";
    let cfg = I18nConfig::parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en"]);
    assert_eq!(cfg.load_path, "./locales");
}

#[test]
fn test_parse_with_metadata() {
    let contents = r#"
        [package.metadata.i18n]
        default-locale = "en"
        available-locales = ["zh-CN"]
        load-path = "./my-locales"
        fallback = ["zh"]
        minify-key = true
        minify-key-len = 12
        minify-key-prefix = "T_"
        minify-key-thresh = 16
    "#;

    let cfg = I18nConfig::parse(contents).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
    assert_eq!(cfg.load_path, "./my-locales");
    assert_eq!(cfg.fallback, vec!["zh"]);
    assert!(cfg.minify_key);
    assert_eq!(cfg.minify_key_len, 12);
    assert_eq!(cfg.minify_key_prefix, "T_");
    assert_eq!(cfg.minify_key_thresh, 16);
}

#[test]
fn test_load_default() {
    let workdir = Path::new(env!["CARGO_MANIFEST_DIR"]);

    let cfg = I18nConfig::load(workdir).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en"]);
    assert_eq!(cfg.load_path, "./locales");
}

#[test]
fn test_load() {
    let workdir = Path::new(env!["CARGO_MANIFEST_DIR"]);
    let cargo_root = workdir.join("../../examples/foo");

    let cfg = I18nConfig::load(&cargo_root).unwrap();
    assert_eq!(cfg.default_locale, "en");
    assert_eq!(cfg.available_locales, vec!["en", "zh-CN"]);
}

use rstest::rstest;
use serde::Deserialize;
use serde_xml_rs::from_reader;
use std::fs::File;

#[derive(Debug, PartialEq, Deserialize)]
struct Rss {
    channel: Channel,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Channel {
    title: String,
    description: String,
    link: String,
    image: Image,
    generator: String,
    last_build_date: String,
    #[serde(rename = "atom:link")]
    atom_link: AtomLink,
    pub_date: String,
    #[serde(rename = "snf:logo")]
    snf_logo: String,
    #[serde(rename = "item")]
    items: Vec<Item>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct AtomLink {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@rel")]
    rel: String,
    #[serde(rename = "@type")]
    r#type: String,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    title: String,
    description: Option<String>,
    link: String,
    guid: Guid,
    #[serde(rename = "category")]
    categories: Vec<String>,
    #[serde(rename = "dc:creator")]
    creator: Option<String>,
    pub_date: String,
    image: Option<Image>,
    #[serde(rename = "content:encoded")]
    content_encoded: String,
    #[serde(rename = "media:thumbnail")]
    media_thumbnail: MediaThumbnail,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Guid {
    #[serde(rename = "@isPermaLink")]
    is_perma_link: bool,
    #[serde(rename = "#text")]
    url: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Image {
    url: Option<String>,
    title: Option<String>,
    link: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct MediaThumbnail {
    #[serde(rename = "@url")]
    url: String,
}

#[rstest]
#[test_log::test]
fn when_deserialize() {
    let file = File::open("tests/hackernoon_rss/feed.rss.xml").unwrap();
    let _value: Rss = from_reader(file).unwrap();
}

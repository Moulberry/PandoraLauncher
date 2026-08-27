// DWA channel classification rules (`Classifier` in
// internal_dwa_classifier.h): the built-in encoder/legacy tables and the
// serialization used by version >= 2 chunks.

use std::{borrow::Cow, convert::TryInto};

use super::{channel_suffix, CompressorScheme};
use crate::{
    error::{Error, Result},
    meta::attribute::{ChannelList, SampleType},
};

/// One channel classification rule (`Classifier` in
/// internal_dwa_classifier.h): matches a channel by name suffix and sample
/// type, and assigns its compression scheme.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Rule {
    suffix: Cow<'static, str>,
    pub(super) scheme: CompressorScheme,
    sample_type: SampleType,
    /// "Some(0/1/2)" marks this suffix as the R/G/B member of a potential
    /// CSC triplet; "None" (like Y/RY/BY/A) is never CSC-grouped.
    pub(super) csc_index: Option<usize>,
    case_insensitive: bool,
}

impl Rule {
    /// "Classifier_match" exact suffix comparison, plus type equality.
    pub(super) fn matches(&self, suffix: &str, sample_type: SampleType) -> bool {
        self.sample_type == sample_type
            && (if self.case_insensitive {
                suffix.eq_ignore_ascii_case(&self.suffix)
            } else {
                suffix == self.suffix
            })
    }

    fn serialized_size(&self) -> usize {
        self.suffix.len() + 1 + 2
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(self.suffix.as_bytes());
        out.push(0);

        let csc_bits = match self.csc_index {
            None => 0,
            Some(index @ 0..=2) => index + 1,
            Some(_) => return Err(Error::invalid("DWA channel rule csc index out of range")),
        };

        let scheme_bits = match self.scheme {
            CompressorScheme::Unknown => 0,
            CompressorScheme::LossyDct => 1,
            CompressorScheme::Rle => 2,
        };

        let sample_type = match self.sample_type {
            SampleType::U32 => 0,
            SampleType::F16 => 1,
            SampleType::F32 => 2,
        };

        out.push(
            ((csc_bits as u8) << 4) | ((scheme_bits as u8) << 2) | u8::from(self.case_insensitive),
        );
        out.push(sample_type);
        Ok(())
    }
}

/// Current OpenEXR encoder rules for version-2 chunks. Unlike the legacy
/// decoder fallback, these are case-sensitive and use canonical uppercase
/// channel suffixes.
pub(super) fn default_channel_rules() -> Vec<Rule> {
    // OpenEXR's current encoder emits a small canonical rule table rather
    // than serializing the whole channel list. Only channels matching one of
    // these suffix/type pairs need to be recorded in the chunk header.
    let lossy: [(&'static str, Option<usize>); 6] =
        [("R", Some(0)), ("G", Some(1)), ("B", Some(2)), ("Y", None), ("BY", None), ("RY", None)];

    let mut rules = Vec::with_capacity(15);
    for (suffix, csc_index) in lossy {
        for sample_type in [SampleType::F16, SampleType::F32] {
            rules.push(Rule {
                suffix: Cow::Borrowed(suffix),
                scheme: CompressorScheme::LossyDct,
                sample_type,
                csc_index,
                case_insensitive: false,
            });
        }
    }
    for sample_type in [SampleType::U32, SampleType::F16, SampleType::F32] {
        rules.push(Rule {
            suffix: Cow::Borrowed("A"),
            scheme: CompressorScheme::Rle,
            sample_type,
            csc_index: None,
            case_insensitive: false,
        });
    }
    rules
}

/// "sLegacyChannelRules", implied by chunk versions <2.
pub(super) fn legacy_channel_rules() -> Vec<Rule> {
    // Version < 2 chunks relied on the older mixed-case naming conventions.
    // Keep those rules around so the decoder can still read historical files.
    let lossy: [(&'static str, Option<usize>); 11] = [
        ("r", Some(0)),
        ("red", Some(0)),
        ("g", Some(1)),
        ("grn", Some(1)),
        ("green", Some(1)),
        ("b", Some(2)),
        ("blu", Some(2)),
        ("blue", Some(2)),
        ("y", None),
        ("by", None),
        ("ry", None),
    ];

    let mut rules = Vec::with_capacity(25);
    for (suffix, csc_index) in lossy {
        for sample_type in [SampleType::F16, SampleType::F32] {
            rules.push(Rule {
                suffix: Cow::Borrowed(suffix),
                scheme: CompressorScheme::LossyDct,
                sample_type,
                csc_index,
                case_insensitive: true,
            });
        }
    }
    for sample_type in [SampleType::U32, SampleType::F16, SampleType::F32] {
        rules.push(Rule {
            suffix: Cow::Borrowed("a"),
            scheme: CompressorScheme::Rle,
            sample_type,
            csc_index: None,
            case_insensitive: true,
        });
    }
    rules
}

/// Version >= 2 chunks embed the rules they were encoded with, prefixed by
/// a u16 little-endian total size that includes the size field itself
/// (`DwaCompressor_readChannelRules`).
pub(super) fn parse_channel_rules(input: &mut &[u8]) -> Result<Vec<Rule>> {
    // The serialized rule block is prefixed by a u16 size that includes the
    // size field itself, so the parser can skip over the whole table at once.
    let (size_bytes, rest) = input
        .split_first_chunk::<2>()
        .ok_or_else(|| Error::invalid("truncated DWA channel rules"))?;
    let total_size = u16::from_le_bytes(*size_bytes) as usize;

    let rules_size =
        total_size.checked_sub(2).ok_or_else(|| Error::invalid("truncated DWA channel rules"))?;
    if rules_size > rest.len() {
        return Err(Error::invalid("truncated DWA channel rules"));
    }

    let mut rules_data = &rest[..rules_size];
    *input = &rest[rules_size..];

    let mut rules = Vec::new();
    while !rules_data.is_empty() {
        rules.push(parse_rule(&mut rules_data)?);
    }
    Ok(rules)
}

/// One serialized rule ("Classifier_read"): a NUL-terminated suffix
/// (at most 128 chars), a packed flags byte, and a pixel type byte.
fn parse_rule(data: &mut &[u8]) -> Result<Rule> {
    let corrupt = || Error::invalid("corrupt DWA channel rule");

    let suffix_len = data.iter().position(|&byte| byte == 0).ok_or_else(corrupt)?;
    if suffix_len > 128 {
        return Err(corrupt());
    }
    let suffix = String::from_utf8_lossy(&data[..suffix_len]).into_owned();

    let rest = &data[suffix_len + 1..];
    let (chunk, rest) = rest.split_first_chunk::<2>().ok_or_else(corrupt)?;
    let [flags, type_byte] = *chunk;
    *data = rest;

    Ok(Rule {
        suffix: Cow::Owned(suffix),
        // The packed flags byte matches the C reference layout:
        // high nibble = cscIdx + 1, bits 2-3 = scheme, bit 0 = case-insensitive.
        csc_index: match ((flags >> 4) as i32) - 1 {
            -1 => None,
            index @ 0..=2 => Some(index as usize),
            _ => {
                return Err(corrupt());
            }
        },
        scheme: match (flags >> 2) & 3 {
            0 => CompressorScheme::Unknown,
            1 => CompressorScheme::LossyDct,
            2 => CompressorScheme::Rle,
            _ => {
                return Err(corrupt());
            }
        },
        case_insensitive: (flags & 1) != 0,
        sample_type: match type_byte {
            0 => SampleType::U32,
            1 => SampleType::F16,
            2 => SampleType::F32,
            _ => {
                return Err(corrupt());
            }
        },
    })
}

/// Encoder-side companion to `parse_channel_rules`: writes a u16 byte count
/// including the size field itself, followed by only the default rules that
/// match at least one channel in this chunk's channel list.
pub(super) fn write_relevant_channel_rules(
    rules: &[Rule],
    channels: &ChannelList,
) -> Result<Vec<u8>> {
    // The encoder only writes rules that are actually used by at least one
    // channel in this chunk. This keeps the chunk leader self-contained while
    // avoiding dead table entries.
    let mut payload = Vec::new();

    for rule in rules {
        let relevant = channels.list.iter().any(|channel| {
            let name = channel.name.to_string();
            rule.matches(channel_suffix(&name), channel.sample_type)
        });

        if relevant {
            payload.reserve(rule.serialized_size());
            rule.write(&mut payload)?;
        }
    }

    let total_size = payload
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::invalid("DWA channel rules too large"))?;
    let total_size: u16 =
        total_size.try_into().map_err(|_| Error::invalid("DWA channel rules too large"))?;

    let mut out = Vec::with_capacity(total_size as usize);
    out.extend_from_slice(&total_size.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

#[cfg(test)]
mod test {
    use smallvec::smallvec;

    use super::*;
    use crate::meta::attribute::ChannelDescription;

    /// Writing a single rule and parsing it back must preserve every field.
    #[test]
    fn single_rule_write_parse_roundtrip() {
        for original in default_channel_rules().into_iter().chain(legacy_channel_rules()) {
            let mut bytes = Vec::new();
            original.write(&mut bytes).unwrap();
            assert_eq!(bytes.len(), original.serialized_size());

            let mut data = bytes.as_slice();
            let parsed = parse_rule(&mut data).unwrap();

            assert_eq!(parsed, original);
            assert!(data.is_empty(), "parse_rule must consume exactly one rule");
        }
    }

    /// The u16-size-prefixed relevant-rules block must survive a
    /// write -> parse -> re-write roundtrip byte-for-byte, which also proves
    /// `parse_channel_rules` and `parse_rule` agree with the writer.
    #[test]
    fn relevant_rules_block_roundtrip() {
        let channels = ChannelList::new(smallvec![
            ChannelDescription::named("R", SampleType::F16),
            ChannelDescription::named("G", SampleType::F16),
            ChannelDescription::named("B", SampleType::F16),
            ChannelDescription::named("A", SampleType::F16),
        ]);

        let block = write_relevant_channel_rules(&default_channel_rules(), &channels).unwrap();

        let mut input = block.as_slice();
        let parsed = parse_channel_rules(&mut input).unwrap();
        assert!(input.is_empty(), "the block is fully described by its u16 size prefix");
        assert!(!parsed.is_empty(), "R/G/B/A channels must match at least one rule");

        // Re-serialize the parsed rules in the same u16-size-prefixed form and
        // require it to be identical to what the encoder wrote.
        let mut payload = Vec::new();
        for rule in &parsed {
            rule.write(&mut payload).unwrap();
        }
        let mut rebuilt = Vec::new();
        rebuilt.extend_from_slice(&((payload.len() + 2) as u16).to_le_bytes());
        rebuilt.extend_from_slice(&payload);

        assert_eq!(rebuilt, block);
    }
}

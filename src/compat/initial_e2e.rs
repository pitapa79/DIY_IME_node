//! Issue #1 の初期統合テスト用ノード。小さな固定規則・辞書でデータの連携を検証する。
use super::{config_keys, output};
use crate::{
    Candidate, ChoiceOption, Packet, TextMeta,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
    tree::map_text,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub const ROMAJI: &str = "honjitunokousuikakurituha30%desu";
pub const HIRAGANA: &str = "ほんじつのこうすいかくりつは30%です";
pub const EXPECTED: &str = "本日の降水確率は30%です";

fn port(id: &str, language: &str, root: RootConstraint) -> Port {
    Port::packet(id).with_language(language).with_root(root)
}
fn unary(
    kind: &str,
    input: RootConstraint,
    result: RootConstraint,
    input_language: &str,
) -> NodeDefinition {
    NodeDefinition::new(
        kind,
        1,
        vec![InputPort::required(port("in", input_language, input))],
        vec![port("out", "ja", result)],
    )
}
fn packet(inputs: &Inputs) -> Result<Option<&Packet>, CoreError> {
    match inputs.get("in") {
        Some(OutputValue::Data(packet)) => {
            packet
                .validate()
                .map_err(|error| CoreError::new("invalid-packet", error.to_string()))?;
            Ok(Some(packet))
        }
        Some(OutputValue::NoOutput) => Ok(None),
        None => Err(CoreError::new("missing-input", "in is required")),
    }
}
fn text(inputs: &Inputs) -> Result<Option<&str>, CoreError> {
    match packet(inputs)? {
        Some(Packet {
            root: Candidate::Text { text, .. },
            ..
        }) => Ok(Some(text)),
        Some(_) => Err(CoreError::new(
            "invalid-root",
            "this node requires a Text root",
        )),
        None => Ok(None),
    }
}
fn data(root: Candidate) -> Outputs {
    output(OutputValue::Data(Arc::new(Packet::new(root))))
}

// 長い規則を優先する。対応しない英字は成功した文字列へ混ぜず、エラーにする。
const ROMAJI_RULES: &[(&str, &str)] = &[
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("sa", "さ"),
    ("si", "し"),
    ("shi", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("ta", "た"),
    ("ti", "ち"),
    ("chi", "ち"),
    ("tu", "つ"),
    ("tsu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("fu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("za", "ざ"),
    ("zi", "じ"),
    ("ji", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    ("kya", "きゃ"),
    ("kyu", "きゅ"),
    ("kyo", "きょ"),
    ("gya", "ぎゃ"),
    ("gyu", "ぎゅ"),
    ("gyo", "ぎょ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("sho", "しょ"),
    ("sya", "しゃ"),
    ("syu", "しゅ"),
    ("syo", "しょ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("tya", "ちゃ"),
    ("tyu", "ちゅ"),
    ("tyo", "ちょ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("jo", "じょ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("nya", "にゃ"),
    ("nyu", "にゅ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyu", "ひゅ"),
    ("hyo", "ひょ"),
    ("mya", "みゃ"),
    ("myu", "みゅ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryu", "りゅ"),
    ("ryo", "りょ"),
    ("bya", "びゃ"),
    ("byu", "びゅ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyu", "ぴゅ"),
    ("pyo", "ぴょ"),
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("xtu", "っ"),
    ("xtsu", "っ"),
];

fn to_hiragana(input: &str) -> Result<String, CoreError> {
    let lower = input.to_ascii_lowercase();
    let mut offset = 0;
    let mut result = String::new();
    while offset < lower.len() {
        let remaining = &lower[offset..];
        let mut chars = remaining.chars();
        let current = chars.next().unwrap();
        let next = chars.next();
        if !current.is_ascii_alphabetic() {
            result.push(current);
            offset += current.len_utf8();
            continue;
        }
        if current == 'n' {
            if next == Some('\'') {
                result.push('ん');
                offset += 2;
                continue;
            }
            if next == Some('n') {
                result.push('ん');
                let after = chars.next();
                offset += if after.is_some_and(|ch| "aiueoy".contains(ch)) {
                    1
                } else {
                    2
                };
                continue;
            }
            if next.is_none_or(|ch| !"aiueoy".contains(ch)) {
                result.push('ん');
                offset += 1;
                continue;
            }
        }
        if next == Some(current)
            && !"aiueon".contains(current)
            && ROMAJI_RULES
                .iter()
                .any(|(rule, _)| rule.starts_with(current))
        {
            result.push('っ');
            offset += 1;
            continue;
        }
        if let Some((roman, kana)) = ROMAJI_RULES
            .iter()
            .filter(|(roman, _)| remaining.starts_with(roman))
            .max_by_key(|(roman, _)| roman.len())
        {
            result.push_str(kana);
            offset += roman.len();
        } else {
            return Err(CoreError::new(
                "unsupported-romaji",
                format!(
                    "cannot convert romaji at code point {}: {remaining}",
                    lower[..offset].chars().count()
                ),
            ));
        }
    }
    Ok(result)
}

pub struct RomajiToHiragana;
impl Node for RomajiToHiragana {
    fn definition(&self) -> NodeDefinition {
        unary(
            "e2e.romaji-to-hiragana",
            RootConstraint::Text,
            RootConstraint::Text,
            "ja-Latn",
        )
    }
    fn execute(&self, _: RunContext<'_>, inputs: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        let Some(input) = text(inputs)? else {
            return Ok(output(OutputValue::NoOutput));
        };
        Ok(data(Candidate::Text {
            text: to_hiragana(input)?,
            meta: None,
        }))
    }
}

const WORDS: &[&str] = &[
    "こうすいかくりつ",
    "ほんじつ",
    "こうすい",
    "かくりつ",
    "りつ",
    "です",
    "の",
    "は",
];
fn split(input: &str, compound: bool) -> Candidate {
    let chars: Vec<_> = input.chars().collect();
    let mut cursor = 0;
    let mut items = Vec::new();
    while cursor < chars.len() {
        let start = cursor;
        if chars[cursor].is_ascii_digit() {
            while cursor < chars.len() && (chars[cursor].is_ascii_digit() || chars[cursor] == '.') {
                cursor += 1;
            }
            if chars.get(cursor) == Some(&'%') {
                cursor += 1;
            }
        } else {
            let length = WORDS
                .iter()
                .filter(|word| compound || **word != "こうすいかくりつ")
                .filter(|word| {
                    chars[start..]
                        .iter()
                        .copied()
                        .zip(word.chars())
                        .all(|(a, b)| a == b)
                        && word.chars().count() <= chars.len() - start
                })
                .map(|word| word.chars().count())
                .max()
                .unwrap_or(1);
            cursor += length;
        }
        let value: String = chars[start..cursor].iter().collect();
        items.push(Candidate::Text {
            text: value.clone(),
            meta: Some(TextMeta {
                reading: Some(value),
                source_span: Some([start, cursor]),
            }),
        });
    }
    Candidate::Sequence { items }
}

pub struct Segment;
impl Node for Segment {
    fn definition(&self) -> NodeDefinition {
        unary(
            "e2e.segment",
            RootConstraint::Text,
            RootConstraint::Choice,
            "ja",
        )
    }
    fn execute(&self, _: RunContext<'_>, inputs: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        let Some(input) = text(inputs)? else {
            return Ok(output(OutputValue::NoOutput));
        };
        let primary = split(input, true);
        let alternate = split(input, false);
        let mut options = vec![ChoiceOption {
            value: primary,
            score: Some(85.0),
        }];
        if alternate != options[0].value {
            options.push(ChoiceOption {
                value: alternate,
                score: Some(15.0),
            });
        }
        Ok(data(Candidate::Choice { options }))
    }
}

pub struct KanaKanjiCandidates;
impl Node for KanaKanjiCandidates {
    fn definition(&self) -> NodeDefinition {
        unary(
            "e2e.kana-kanji",
            RootConstraint::Any,
            RootConstraint::Any,
            "ja",
        )
    }
    fn execute(&self, _: RunContext<'_>, inputs: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        let Some(input) = packet(inputs)? else {
            return Ok(output(OutputValue::NoOutput));
        };
        let root = map_text(&input.root, |reading, meta| {
            let variants: Option<&[(&str, f64)]> = match reading {
                "ほんじつ" => Some(&[("本日", 100.0), ("ほんじつ", 10.0)]),
                "こうすいかくりつ" => Some(&[("降水確率", 100.0), ("降水確立", 20.0)]),
                "こうすい" => Some(&[("降水", 100.0), ("こうすい", 10.0)]),
                "かくりつ" => Some(&[("確率", 100.0), ("確立", 20.0)]),
                "りつ" => Some(&[("率", 100.0), ("りつ", 10.0)]),
                _ => None,
            };
            match variants {
                Some(variants) => Candidate::Choice {
                    options: variants
                        .iter()
                        .map(|(value, score)| ChoiceOption {
                            value: Candidate::Text {
                                text: (*value).into(),
                                meta: Some(TextMeta {
                                    reading: Some(reading.into()),
                                    source_span: meta.and_then(|meta| meta.source_span),
                                }),
                            },
                            score: Some(*score),
                        })
                        .collect(),
                },
                None => Candidate::Text {
                    text: reading.into(),
                    meta: meta.cloned(),
                },
            }
        });
        Ok(data(root))
    }
}

fn choose(root: &Candidate, unscored: f64) -> Option<Candidate> {
    enum Frame<'a> {
        Visit(&'a Candidate),
        Sequence(usize),
    }
    let mut frames = vec![Frame::Visit(root)];
    let mut values = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(Candidate::Text { text, meta }) => values.push(Candidate::Text {
                text: text.clone(),
                meta: meta.clone(),
            }),
            Frame::Visit(Candidate::Sequence { items }) => {
                frames.push(Frame::Sequence(items.len()));
                frames.extend(items.iter().rev().map(Frame::Visit));
            }
            Frame::Visit(Candidate::Choice { options }) => {
                let mut best: Option<&ChoiceOption> = None;
                for option in options {
                    if best.is_none_or(|current| {
                        option.score.unwrap_or(unscored) > current.score.unwrap_or(unscored)
                    }) {
                        best = Some(option);
                    }
                }
                frames.push(Frame::Visit(&best?.value));
            }
            Frame::Sequence(count) => {
                let items = values.split_off(values.len() - count);
                values.push(Candidate::Sequence { items });
            }
        }
    }
    values.pop()
}

pub struct SelectCandidates;
impl Node for SelectCandidates {
    fn definition(&self) -> NodeDefinition {
        let mut definition = unary(
            "e2e.select-candidates",
            RootConstraint::Any,
            RootConstraint::Any,
            "ja",
        );
        definition.default_config = json!({"unscored":0.0});
        definition.config_schema = json!({"type":"object","required":["unscored"],"additionalProperties":false,"properties":{"unscored":{"type":"number"}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        let object = config_keys(config, &["unscored"])?;
        if object["unscored"].as_f64().is_some_and(f64::is_finite) {
            Ok(())
        } else {
            Err(CoreError::new("invalid-config", "unscored must be finite"))
        }
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        self.validate_config(config)?;
        let Some(input) = packet(inputs)? else {
            return Ok(output(OutputValue::NoOutput));
        };
        Ok(choose(&input.root, config["unscored"].as_f64().unwrap())
            .map_or_else(|| output(OutputValue::NoOutput), data))
    }
}

pub struct Stringify;
impl Node for Stringify {
    fn definition(&self) -> NodeDefinition {
        unary(
            "e2e.stringify",
            RootConstraint::Any,
            RootConstraint::Text,
            "ja",
        )
    }
    fn execute(&self, _: RunContext<'_>, inputs: &Inputs, _: &Value) -> Result<Outputs, CoreError> {
        let Some(input) = packet(inputs)? else {
            return Ok(output(OutputValue::NoOutput));
        };
        let mut pending = vec![&input.root];
        let mut text = String::new();
        while let Some(candidate) = pending.pop() {
            match candidate {
                Candidate::Text { text: part, .. } => text.push_str(part),
                Candidate::Sequence { items } => pending.extend(items.iter().rev()),
                Candidate::Choice { .. } => {
                    return Err(CoreError::new(
                        "unresolved-choice",
                        "select candidates before stringifying",
                    ));
                }
            }
        }
        Ok(data(Candidate::Text { text, meta: None }))
    }
}

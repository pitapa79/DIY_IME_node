//! ローマ字→ひらがなの純粋な変換処理。Node・Engine・実行時入力には依存しない。
//! 契約: docs/node-contracts-v0.1.md の「ローマ字→ひらがな」。
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use serde_json::Value;

use crate::node::CoreError;

/// 既定の変換表。コードから分けた同梱データ（キー<TAB>出力）。
const DEFAULT_TABLE: &str = include_str!("default_table.tsv");

const MAX_KEY_CHARS: usize = 8;
const MAX_TO_CODE_POINTS: usize = 16;
const MAX_OVERRIDES: usize = 256;
/// 制御規則のため overrides の from に指定できない。
const RESERVED_KEYS: [&str; 3] = ["n", "nn", "n'"];

/// 実行モード。実行時入力の compose / finalize と対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 入力途中。未完の末尾は pending に保持する。
    Compose,
    /// 確定。末尾の n は「ん」、残る未完は unknown の方針で扱う。
    Finalize,
}

/// 未知・未完の文字の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    Preserve,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub from: String,
    pub to: String,
}

/// 検査済みの設定。`Config::from_value` 以外で作る場合は `validate` を通す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub overrides: Vec<Override>,
    pub unknown: Unknown,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            overrides: Vec::new(),
            unknown: Unknown::Preserve,
        }
    }
}

fn invalid_config(message: impl Into<String>) -> CoreError {
    CoreError::new("invalid-config", message)
}

impl Config {
    /// 設定JSONを検査して読み込む。2キー（overrides / unknown）が必須で、部分マージはしない。
    pub fn from_value(value: &Value) -> Result<Self, CoreError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_config("expected an object"))?;
        if object.len() != 2 || !object.contains_key("overrides") || !object.contains_key("unknown")
        {
            return Err(invalid_config(
                "expected exactly these settings: [\"overrides\", \"unknown\"]",
            ));
        }
        let unknown = match object["unknown"].as_str() {
            Some("preserve") => Unknown::Preserve,
            Some("error") => Unknown::Error,
            _ => return Err(invalid_config("unknown must be \"preserve\" or \"error\"")),
        };
        let list = object["overrides"]
            .as_array()
            .ok_or_else(|| invalid_config("overrides must be an array"))?;
        if list.len() > MAX_OVERRIDES {
            return Err(invalid_config(format!(
                "overrides allows at most {MAX_OVERRIDES} entries"
            )));
        }
        let mut seen = HashSet::new();
        let mut overrides = Vec::with_capacity(list.len());
        for entry in list {
            let entry = entry
                .as_object()
                .filter(|o| o.len() == 2)
                .ok_or_else(|| invalid_config("each override must be {\"from\",\"to\"}"))?;
            let (Some(from), Some(to)) = (
                entry.get("from").and_then(Value::as_str),
                entry.get("to").and_then(Value::as_str),
            ) else {
                return Err(invalid_config("override from/to must be strings"));
            };
            check_key(from)?;
            let to_len = to.chars().count();
            if to_len == 0 || to_len > MAX_TO_CODE_POINTS {
                return Err(invalid_config(format!(
                    "override to must be 1 to {MAX_TO_CODE_POINTS} code points"
                )));
            }
            if !seen.insert(from.to_string()) {
                return Err(invalid_config(format!("duplicate override from: {from}")));
            }
            overrides.push(Override {
                from: from.into(),
                to: to.into(),
            });
        }
        Ok(Self { overrides, unknown })
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        let mut seen = HashSet::new();
        if self.overrides.len() > MAX_OVERRIDES {
            return Err(invalid_config("too many overrides"));
        }
        for entry in &self.overrides {
            check_key(&entry.from)?;
            let to_len = entry.to.chars().count();
            if to_len == 0 || to_len > MAX_TO_CODE_POINTS || !seen.insert(entry.from.as_str()) {
                return Err(invalid_config("invalid override"));
            }
        }
        Ok(())
    }
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_lowercase() || c == '\''
}

fn check_key(from: &str) -> Result<(), CoreError> {
    let len = from.chars().count();
    if len == 0 || len > MAX_KEY_CHARS || !from.chars().all(is_key_char) {
        return Err(invalid_config(format!(
            "override from must be 1 to {MAX_KEY_CHARS} lowercase ASCII letters or apostrophes"
        )));
    }
    if RESERVED_KEYS.contains(&from) {
        return Err(invalid_config(format!("override from is reserved: {from}")));
    }
    Ok(())
}

/// 既定表と overrides を合成した変換表。
struct Table {
    rules: HashMap<String, String>,
    /// 全キーの真の接頭辞（長さ1以上）。
    prefixes: HashSet<String>,
    /// キーの先頭文字（促音化の対象判定用）。
    initials: HashSet<char>,
}

impl Table {
    fn build(config: &Config) -> Self {
        let mut rules: HashMap<String, String> = DEFAULT_TABLE
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('\t'))
            .map(|(from, to)| (from.to_string(), to.to_string()))
            .collect();
        for entry in &config.overrides {
            rules.insert(entry.from.clone(), entry.to.clone());
        }
        let mut prefixes = HashSet::new();
        let mut initials = HashSet::new();
        for key in rules.keys() {
            let chars: Vec<char> = key.chars().collect();
            initials.insert(chars[0]);
            for end in 1..chars.len() {
                prefixes.insert(chars[..end].iter().collect());
            }
        }
        Self {
            rules,
            prefixes,
            initials,
        }
    }
}

/// 同梱TSVだけから作る不変・有界の表。呼出やインスタンスごとの設定は保持しない。
fn default_table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table::build(&Config::default()))
}

/// 変換結果。`pending` は入力途中の末尾（原文の大小を保持）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub text: String,
    pub pending: String,
}

fn lower(chars: &[char]) -> String {
    chars.iter().map(|c| c.to_ascii_lowercase()).collect()
}

fn is_vowel_or_y(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'i' | 'u' | 'e' | 'o' | 'y')
}

/// 入力の全文スナップショットを変換する。入力・設定は変更しない純粋関数。
pub fn convert(input: &str, mode: Mode, config: &Config) -> Result<Converted, CoreError> {
    config.validate()?;
    // 任意設定の表は共有しない。unknownは各呼出で評価し、既定表の構成へ含めない。
    let custom_table = (!config.overrides.is_empty()).then(|| Table::build(config));
    let table = custom_table.as_ref().unwrap_or_else(|| default_table());
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if !(c.is_ascii_alphabetic() || c == '\'') {
            out.push(c);
            i += 1;
            continue;
        }
        // 1. 最長一致。
        let longest = (1..=MAX_KEY_CHARS.min(chars.len() - i))
            .rev()
            .find_map(|len| {
                let slice = &chars[i..i + len];
                slice
                    .iter()
                    .all(|c| c.is_ascii_alphabetic() || *c == '\'')
                    .then(|| table.rules.get(&lower(slice)).map(|to| (len, to)))
                    .flatten()
            });
        if let Some((len, to)) = longest {
            out.push_str(to);
            i += len;
            continue;
        }
        // 2. n の制御規則（n / nn / n'）。
        if c.eq_ignore_ascii_case(&'n') {
            match chars.get(i + 1).copied() {
                None => {
                    if mode == Mode::Finalize {
                        out.push('ん');
                        i += 1;
                    } else {
                        return Ok(Converted {
                            text: out,
                            pending: chars[i..].iter().collect(),
                        });
                    }
                    continue;
                }
                Some('\'') => {
                    out.push('ん');
                    i += 2;
                    continue;
                }
                Some(next) if next.eq_ignore_ascii_case(&'n') => {
                    // nn は「ん」。ただし2つ目の n が na / nya 等の頭になる場合は1文字だけ消費する。
                    let followed = chars.get(i + 2).copied().is_some_and(is_vowel_or_y);
                    out.push('ん');
                    i += if followed { 1 } else { 2 };
                    continue;
                }
                Some(next) if !is_vowel_or_y(next) => {
                    out.push('ん');
                    i += 1;
                    continue;
                }
                Some(_) => {} // ny 等は未完の接頭辞として下で扱う。
            }
        }
        // 3. 促音。対応する子音の重複だけを対象にする。
        let lc = c.to_ascii_lowercase();
        if c.is_ascii_alphabetic()
            && !matches!(lc, 'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'x' | 'l')
            && table.initials.contains(&lc)
            && chars
                .get(i + 1)
                .is_some_and(|next| next.eq_ignore_ascii_case(&c))
        {
            out.push('っ');
            i += 1;
            continue;
        }
        // 4. 未完の接頭辞。末尾全体が規則の接頭辞なら保持またはfinalizeの方針で扱う。
        let tail = &chars[i..];
        let tail_is_prefix = tail.len() < MAX_KEY_CHARS
            && tail.iter().all(|c| c.is_ascii_alphabetic() || *c == '\'')
            && table.prefixes.contains(&lower(tail));
        if tail_is_prefix {
            match (mode, config.unknown) {
                (Mode::Compose, _) => {
                    return Ok(Converted {
                        text: out,
                        pending: tail.iter().collect(),
                    });
                }
                (Mode::Finalize, Unknown::Preserve) => {
                    out.extend(tail);
                    return Ok(Converted {
                        text: out,
                        pending: String::new(),
                    });
                }
                (Mode::Finalize, Unknown::Error) => {
                    return Err(CoreError::new(
                        "incomplete-romaji",
                        "input ends with an incomplete romaji sequence",
                    ));
                }
            }
        }
        // 5. 未知の文字。アポストロフィ単独は記号としてそのまま通す。
        match config.unknown {
            _ if c == '\'' => out.push(c),
            Unknown::Preserve => out.push(c),
            Unknown::Error => {
                return Err(CoreError::new(
                    "unsupported-romaji",
                    format!("no romaji rule for {c:?}"),
                ));
            }
        }
        i += 1;
    }
    Ok(Converted {
        text: out,
        pending: String::new(),
    })
}

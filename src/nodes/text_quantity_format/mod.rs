//! 未採用の実験ノード。契約はexamples/nodes/text_quantity_format/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-quantity-format";
#[derive(Debug, Clone, Copy, Default)]
pub struct TextQuantityFormat;
fn settings(config: &Value) -> Result<(), CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if !object.is_empty() {
        return Err(CoreError::new(
            "invalid-config",
            "this node declares no settings",
        ));
    }
    Ok(())
}
const DIGITS: [&str; 10] = [
    "ぜろ",
    "いち",
    "に",
    "さん",
    "よん",
    "ご",
    "ろく",
    "なな",
    "はち",
    "きゅう",
];
/// 0..=9999 の読み。0 は「ぜろ」。
fn number_reading(mut n: u32) -> String {
    if n == 0 {
        return DIGITS[0].to_string();
    }
    let mut out = String::new();
    let thousands = n / 1000;
    n %= 1000;
    match thousands {
        0 => {}
        1 => out.push_str("せん"),
        3 => out.push_str("さんぜん"),
        8 => out.push_str("はっせん"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("せん");
        }
    }
    let hundreds = n / 100;
    n %= 100;
    match hundreds {
        0 => {}
        1 => out.push_str("ひゃく"),
        3 => out.push_str("さんびゃく"),
        6 => out.push_str("ろっぴゃく"),
        8 => out.push_str("はっぴゃく"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("ひゃく");
        }
    }
    let tens = n / 10;
    let ones = n % 10;
    match tens {
        0 => {}
        1 => out.push_str("じゅう"),
        d => {
            out.push_str(DIGITS[d as usize]);
            out.push_str("じゅう");
        }
    }
    if ones > 0 {
        out.push_str(DIGITS[ones as usize]);
    }
    out
}
struct Counter {
    /// 促音化しない通常形。
    plain: &'static str,
    /// 末尾が促音化する位置（1・6・8・0終わり）で使う形。通常形と同じ場合もある。
    sokuon: &'static str,
    /// 「ん」で終わる読み（さん・せん等）の後で使う濁音形。ない場合は通常形。
    after_n: Option<&'static str>,
    /// 音変化を一切しない助数詞か。
    fixed: bool,
}
fn counter(name: char) -> Option<Counter> {
    Some(match name {
        '本' => Counter {
            plain: "ほん",
            sokuon: "ぽん",
            after_n: Some("ぼん"),
            fixed: false,
        },
        '個' => Counter {
            plain: "こ",
            sokuon: "こ",
            after_n: None,
            fixed: false,
        },
        '冊' => Counter {
            plain: "さつ",
            sokuon: "さつ",
            after_n: None,
            fixed: false,
        },
        '匹' => Counter {
            plain: "ひき",
            sokuon: "ぴき",
            after_n: Some("びき"),
            fixed: false,
        },
        '回' => Counter {
            plain: "かい",
            sokuon: "かい",
            after_n: None,
            fixed: false,
        },
        '枚' => Counter {
            plain: "まい",
            sokuon: "まい",
            after_n: None,
            fixed: true,
        },
        '台' => Counter {
            plain: "だい",
            sokuon: "だい",
            after_n: None,
            fixed: true,
        },
        _ => return None,
    })
}
fn quantity_reading(n: u32, name: char) -> Option<String> {
    if name == '人' {
        return Some(match n {
            1 => "ひとり".to_string(),
            2 => "ふたり".to_string(),
            0 => "ぜろにん".to_string(),
            _ => {
                let base = number_reading(n);
                if let Some(stripped) = base.strip_suffix("よん") {
                    format!("{stripped}よにん")
                } else {
                    format!("{base}にん")
                }
            }
        });
    }
    let counter = counter(name)?;
    let base = number_reading(n);
    if n == 0 || counter.fixed {
        return Some(format!("{base}{}", counter.plain));
    }
    for (tail, cut) in [("いち", "いっ"), ("ろく", "ろっ"), ("はち", "はっ")] {
        if let Some(stripped) = base.strip_suffix(tail) {
            return Some(format!("{stripped}{cut}{}", counter.sokuon));
        }
    }
    if let Some(stripped) = base.strip_suffix("じゅう") {
        return Some(format!("{stripped}じゅっ{}", counter.sokuon));
    }
    for tail in ["ひゃく", "びゃく", "ぴゃく"] {
        if let Some(stripped) = base.strip_suffix(tail) {
            let head = &tail[..tail.len() - "く".len()];
            return Some(format!("{stripped}{head}っ{}", counter.sokuon));
        }
    }
    if base.ends_with('ん') {
        return Some(format!(
            "{base}{}",
            counter.after_n.unwrap_or(counter.plain)
        ));
    }
    Some(format!("{base}{}", counter.plain))
}
fn format_quantity(text: &str) -> Result<String, CoreError> {
    let digit_len = text.chars().take_while(|ch| ch.is_ascii_digit()).count();
    let rest: String = text.chars().skip(digit_len).collect();
    if digit_len == 0 || rest.chars().count() != 1 {
        return Err(CoreError::new(
            "invalid-quantity",
            "expected ASCII digits followed by one counter",
        ));
    }
    let number: u32 = text[..digit_len]
        .parse()
        .map_err(|_| CoreError::new("invalid-quantity", "number is out of range"))?;
    if number > 9999 {
        return Err(CoreError::new("invalid-quantity", "number exceeds 9999"));
    }
    let name = rest.chars().next().unwrap();
    quantity_reading(number, name)
        .ok_or_else(|| CoreError::new("unsupported-counter", "counter is not in the table"))
}
impl Node for TextQuantityFormat {
    fn definition(&self) -> NodeDefinition {
        let port = |id| {
            Port::packet(id)
                .with_language("ja")
                .with_root(RootConstraint::Text)
        };
        let mut definition = NodeDefinition::new(
            KIND,
            1,
            vec![InputPort::required(port("in"))],
            vec![port("out")],
        );
        definition.default_config = json!({});
        definition.config_schema = json!({"type":"object","additionalProperties":false});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config)
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                    text: format_quantity(text)?,
                    meta: None,
                })))
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}

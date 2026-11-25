//! 未採用の実験ノード。契約はexamples/nodes/text_kanji_form/README.md。
use crate::{
    Candidate, Packet,
    node::{
        CoreError, InputPort, Inputs, Node, NodeDefinition, OutputValue, Outputs, Port,
        RootConstraint, RunContext,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
pub const KIND: &str = "experimental.text-kanji-form";
/// 旧字体→新字体の明示対応表。並び順が逆変換の代表形を決める
/// （同じ新字体に複数の旧字体がある場合、最初に現れたものを代表形とする）。
const PAIRS: &[(char, char)] = &[
    ('惡', '悪'),
    ('壓', '圧'),
    ('圍', '囲'),
    ('爲', '為'),
    ('醫', '医'),
    ('飮', '飲'),
    ('隱', '隠'),
    ('營', '営'),
    ('衞', '衛'),
    ('驛', '駅'),
    ('圓', '円'),
    ('鹽', '塩'),
    ('緣', '縁'),
    ('應', '応'),
    ('櫻', '桜'),
    ('奧', '奥'),
    ('橫', '横'),
    ('溫', '温'),
    ('穩', '穏'),
    ('假', '仮'),
    ('價', '価'),
    ('會', '会'),
    ('廻', '回'),
    ('壞', '壊'),
    ('樂', '楽'),
    ('渴', '渇'),
    ('卷', '巻'),
    ('關', '関'),
    ('歡', '歓'),
    ('觀', '観'),
    ('顏', '顔'),
    ('歸', '帰'),
    ('氣', '気'),
    ('龜', '亀'),
    ('戲', '戯'),
    ('區', '区'),
    ('勳', '勲'),
    ('經', '経'),
    ('莖', '茎'),
    ('頸', '頚'),
    ('縣', '県'),
    ('儉', '倹'),
    ('劍', '剣'),
    ('險', '険'),
    ('檢', '検'),
    ('國', '国'),
    ('黑', '黒'),
    ('歲', '歳'),
    ('濟', '済'),
    ('劑', '剤'),
    ('册', '冊'),
    ('參', '参'),
    ('產', '産'),
    ('慘', '惨'),
    ('絲', '糸'),
    ('齒', '歯'),
    ('寫', '写'),
    ('釋', '釈'),
    ('壽', '寿'),
    ('收', '収'),
    ('從', '従'),
    ('澁', '渋'),
    ('髓', '髄'),
    ('肅', '粛'),
    ('處', '処'),
    ('證', '証'),
    ('奬', '奨'),
    ('將', '将'),
    ('稱', '称'),
    ('涉', '渉'),
    ('乘', '乗'),
    ('剩', '剰'),
    ('觸', '触'),
    ('緖', '緒'),
    ('潛', '潜'),
    ('淺', '浅'),
    ('踐', '践'),
    ('錢', '銭'),
    ('雙', '双'),
    ('體', '体'),
    ('帶', '帯'),
    ('滯', '滞'),
    ('擇', '択'),
    ('澤', '沢'),
    ('擔', '担'),
    ('膽', '胆'),
    ('團', '団'),
    ('斷', '断'),
    ('彈', '弾'),
    ('遲', '遅'),
    ('癡', '痴'),
    ('蟲', '虫'),
    ('廳', '庁'),
    ('遞', '逓'),
    ('點', '点'),
    ('黨', '党'),
    ('盜', '盗'),
    ('德', '徳'),
    ('讀', '読'),
    ('獨', '独'),
    ('廢', '廃'),
    ('拜', '拝'),
    ('賣', '売'),
    ('麥', '麦'),
    ('發', '発'),
    ('髮', '髪'),
    ('拔', '抜'),
    ('蠻', '蛮'),
    ('祕', '秘'),
    ('濱', '浜'),
    ('佛', '仏'),
    ('拂', '払'),
    ('倂', '併'),
    ('竝', '並'),
    ('步', '歩'),
    ('變', '変'),
    ('寶', '宝'),
    ('峯', '峰'),
    ('豐', '豊'),
    ('萬', '万'),
    ('滿', '満'),
    ('默', '黙'),
    ('譯', '訳'),
    ('豫', '予'),
    ('餘', '余'),
    ('譽', '誉'),
    ('搖', '揺'),
    ('樣', '様'),
    ('謠', '謡'),
    ('來', '来'),
    ('亂', '乱'),
    ('覽', '覧'),
    ('兩', '両'),
    ('獵', '猟'),
    ('禮', '礼'),
    ('鍊', '錬'),
    ('歷', '歴'),
    ('曆', '暦'),
    ('郞', '郎'),
    ('灣', '湾'),
    ('辨', '弁'),
    ('瓣', '弁'),
    ('辯', '弁'),
    ('邊', '辺'),
    ('邉', '辺'),
    ('缺', '欠'),
    ('藝', '芸'),
    ('曉', '暁'),
    ('啞', '唖'),
    ('驅', '駆'),
    ('徑', '径'),
    ('揭', '掲'),
    ('輕', '軽'),
    ('擊', '撃'),
    ('顯', '顕'),
    ('嚴', '厳'),
    ('效', '効'),
    ('廣', '広'),
    ('鑛', '鉱'),
    ('號', '号'),
    ('碎', '砕'),
    ('齋', '斎'),
    ('雜', '雑'),
    ('殘', '残'),
    ('兒', '児'),
    ('實', '実'),
    ('淨', '浄'),
    ('繩', '縄'),
    ('脣', '唇'),
    ('寢', '寝'),
    ('愼', '慎'),
    ('眞', '真'),
    ('盡', '尽'),
    ('圖', '図'),
    ('粹', '粋'),
    ('聲', '声'),
    ('靜', '静'),
    ('齊', '斉'),
    ('攝', '摂'),
    ('爭', '争'),
    ('插', '挿'),
    ('總', '総'),
    ('屬', '属'),
    ('續', '続'),
    ('墮', '堕'),
    ('對', '対'),
    ('單', '単'),
    ('稻', '稲'),
    ('亞', '亜'),
    ('壹', '壱'),
    ('榮', '栄'),
    ('歐', '欧'),
    ('毆', '殴'),
    ('懷', '懐'),
    ('繪', '絵'),
    ('覺', '覚'),
    ('罐', '缶'),
    ('勸', '勧'),
    ('寬', '寛'),
    ('惠', '恵'),
    ('犧', '犠'),
    ('舊', '旧'),
    ('據', '拠'),
    ('虛', '虚'),
    ('堯', '尭'),
    ('條', '条'),
    ('讓', '譲'),
    ('孃', '嬢'),
    ('釀', '醸'),
    ('囑', '嘱'),
    ('巢', '巣'),
    ('樞', '枢'),
    ('稅', '税'),
    ('瘦', '痩'),
    ('藥', '薬'),
    ('畫', '画'),
    ('鐵', '鉄'),
    ('傳', '伝'),
    ('臟', '臓'),
    ('敕', '勅'),
    ('學', '学'),
    ('辭', '辞'),
    ('囊', '嚢'),
    ('龍', '竜'),
    ('瀧', '滝'),
    ('螢', '蛍'),
    ('竊', '窃'),
    ('鬪', '闘'),
    ('頰', '頬'),
    ('貳', '弐'),
    ('黃', '黄'),
    ('麵', '麺'),
    ('檜', '桧'),
    ('蘆', '芦'),
    ('禱', '祷'),
    ('祿', '禄'),
    ('徵', '徴'),
    ('巖', '巌'),
    ('峽', '峡'),
    ('挾', '挟'),
    ('狹', '狭'),
];
#[derive(Debug, Clone, Copy, Default)]
pub struct TextKanjiForm;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    direction: String,
}
fn settings(config: &Value) -> Result<bool, CoreError> {
    let object = config
        .as_object()
        .ok_or_else(|| CoreError::new("invalid-config", "expected an object"))?;
    if object.len() != 1 || !object.contains_key("direction") {
        return Err(CoreError::new(
            "invalid-config",
            "expected every declared setting and no unknown keys",
        ));
    }
    let settings: Settings = serde_json::from_value(config.clone())
        .map_err(|e| CoreError::new("invalid-config", e.to_string()))?;
    match settings.direction.as_str() {
        "to-shinjitai" => Ok(true),
        "to-kyujitai" => Ok(false),
        _ => Err(CoreError::new(
            "invalid-config",
            "direction must be to-shinjitai or to-kyujitai",
        )),
    }
}
fn convert(text: &str, to_shinjitai: bool) -> String {
    text.chars()
        .map(|ch| {
            if to_shinjitai {
                PAIRS
                    .iter()
                    .find(|(kyu, _)| *kyu == ch)
                    .map(|(_, shin)| *shin)
                    .unwrap_or(ch)
            } else {
                PAIRS
                    .iter()
                    .find(|(_, shin)| *shin == ch)
                    .map(|(kyu, _)| *kyu)
                    .unwrap_or(ch)
            }
        })
        .collect()
}
impl Node for TextKanjiForm {
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
        definition.default_config = json!({"direction":"to-shinjitai"});
        definition.config_schema = json!({"type":"object","additionalProperties":false,"required":["direction"],"properties":{"direction":{"enum":["to-shinjitai","to-kyujitai"]}}});
        definition
    }
    fn validate_config(&self, config: &Value) -> Result<(), CoreError> {
        settings(config).map(|_| ())
    }
    fn execute(
        &self,
        _: RunContext<'_>,
        inputs: &Inputs,
        config: &Value,
    ) -> Result<Outputs, CoreError> {
        let to_shinjitai = settings(config)?;
        let output = match inputs.get("in") {
            Some(OutputValue::Data(packet)) => {
                packet
                    .validate()
                    .map_err(|e| CoreError::new("invalid-packet", e.to_string()))?;
                let Candidate::Text { text, .. } = &packet.root else {
                    return Err(CoreError::new("invalid-root", "expected Text"));
                };
                let converted = convert(text, to_shinjitai);
                if converted == *text {
                    OutputValue::Data(packet.clone())
                } else {
                    OutputValue::Data(Arc::new(Packet::new(Candidate::Text {
                        text: converted,
                        meta: None,
                    })))
                }
            }
            Some(OutputValue::NoOutput) => OutputValue::NoOutput,
            None => return Err(CoreError::new("missing-input", "required input: in")),
        };
        Ok([("out".into(), output)].into())
    }
}

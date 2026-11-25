# 文字体系判定（実験）

`experimental.text-script-classify@1` / `TextScriptClassify`。管理Issue #36 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。出力は分類ラベルの Text。入力の meta は引き継がない（ラベルは入力についての新しい事実）。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- ラベルは `hiragana` / `katakana` / `kanji` / `latin` / `digit` / `mixed` / `neutral` / `empty`。
- 文字クラス: ひらがな（U+3040〜309F）、カタカナ（U+30A0〜30FF と半角カナ U+FF66〜FF9D）、漢字（U+3400〜4DBF、U+4E00〜9FFF、々・〆）、ラテン（A-Z、a-z、全角英字）、数字（0-9、全角数字）。空白・句読点・記号・絵文字は中立として判定から除く。
- 結合文字とバリエーションセレクタは直前の文字のクラスを引き継ぐ（か+結合濁点は hiragana）。
- 非中立クラスがちょうど1種ならそのラベル、2種以上なら mixed、中立のみなら neutral、空入力なら empty。文字体系の判定であり、自然言語の推定はしない。
- 文字クラスの定義は text-script-split と揃えている。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_script_classify`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #55（管理Issue: #36）

# 記号表記変換（実験）

`experimental.text-symbol-notation@1` / `TextSymbolNotation`。管理Issue #88 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"preset":"japanese"}`。preset は `japanese`（半角記号→和文記号）または `ascii`（逆方向）。未知キー・部分設定・不正な値は invalid-config。
- japanese: ( ) [ ] { } を全角へ変換する。" は 「 」を交互に、' は 『 』を交互に割り当てる（開閉の対処理）。
- ascii: （ ）［ ］｛ ｝を半角へ、「 」を " へ、『 』を ' へ戻す。
- 境界の事前確認: text.mapping（#17）は固定文字列の置換で対処理やプリセット体系を持たず、既存の句読点系ノードは句読点のみが対象のため、別kindとした（作業Issue #102 に記録）。
- 変換で変化しない場合は入力Packetをそのまま返す（meta保持）。変化した場合は新しい Text（meta なし）。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_symbol_notation`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #102（管理Issue: #88）

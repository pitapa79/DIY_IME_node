# 文字体系別区間分割（実験）

`experimental.text-script-split@1` / `TextScriptSplit`。管理Issue #37 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Text。出力 `out`: ja / Sequence。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- 各項目は文字体系ごとの最大連続区間の Text で、位置は meta の source_span（コードポイント単位の半開区間）で表す。reading は付けない。項目を連結すると原文と一致する（文字を失わない・並べ替えない）。
- 文字クラスは text-script-classify と共通の定義（hiragana / katakana / kanji / latin / digit / other）。空白・句読点・記号・絵文字は other として独立した区間を作る。
- 結合文字とバリエーションセレクタは直前の文字のクラスを引き継ぎ、区間を割らない（先頭の結合文字は other）。
- 空入力は空の Sequence。入力の meta は引き継がない。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_script_split`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #54（管理Issue: #37）

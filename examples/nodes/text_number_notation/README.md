# 数字表記変換（実験）

`experimental.text-number-notation@1` / `TextNumberNotation`。管理Issue #31 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。入力 Text 全体が1つの数値表記である場合だけ変換する（文中の埋め込み変換はしない）。
- 完全設定の既定値: `{"mode":"kanji-positional"}`。`mode` は `kanji-positional`（位取り）/ `kanji-digit`（桁読み）/ `arabic`（漢数字→アラビア数字）。未知キー・部分設定・不正型は invalid-config。
- 範囲: 整数部・小数部とも最大16桁（絶対値 10^16 未満）。負数はアラビア側は先頭の `-`、漢数字側は接頭辞「マイナス」。負のゼロ（-0、-0.0、マイナス零）は invalid-number。
- 位取り: 4桁区切りで万・億・兆を使う。十・百・千の前の「一」は省く（101→百一、1000→千）。0 は零。
- 桁読み: 数字を1桁ずつ漢数字にする（10000→一〇〇〇〇）。0 は 〇。
- 小数部は「点」でつなぎ、両方向とも桁読みする（0.5→零点五、三点一四→3.14）。
- arabic 方向の解釈: 単位字（十百千万億兆）を含む表記は位取りとして、含まない表記は桁読みとして解釈する。「零」は単独の場合だけ 0 とし、位取り表記の中には置けない。先頭ゼロ（007、〇一）は invalid-number。解釈できない入力は推測で確定せず invalid-number エラーにする。空入力も invalid-number。
- Packet検証後に処理する。NoOutputはNoOutput。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_number_notation`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #49（管理Issue: #31）

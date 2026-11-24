# 正規表現置換（実験）

`experimental.text-regex-replace@1` / `TextRegexReplace`。管理Issue #15 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。リテラル置換の `experimental.text-replace` とは別責務。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"pattern":"","replacement":"","mode":"all"}`。`mode` は `all`（全件置換）/ `first`（最初のみ）。未知キー・部分設定・不正型はinvalid-config。
- 正規表現エンジンは `regex` クレート。線形時間で壊滅的なバックトラックがなく、後方参照と先読みは使えない（安全側の制約）。Unicode の文字クラス（`\p{Hiragana}` 等）が使える。
- 置換文字列のキャプチャ参照は regex クレートの展開規則に従う: `$1` / `${1}` / `$name` / `${name}` がグループ参照、リテラルの `$` は `$$` と書く。存在しないグループは空文字へ展開される。
- 上限: pattern は1024 UTF-8バイト以下、replacement は1024 UTF-8バイト以下、コンパイル済みサイズは各1MiBまで。これを超える設定は invalid-config、パターンの文法エラーやサイズ超過は invalid-pattern として計画時に拒否する。
- 結果が1MiBを超える場合は切り捨てず output-limit エラーにする。
- ゼロ幅一致は regex クレートの標準規則に従う（空一致の後に1文字進み、直前の一致の終端と重なる空一致は繰り返さない）。例: `x*` を `-` で置換すると `abx` → `-a-b-`。
- Packet検証後に処理する。NoOutputはNoOutput、空文字はData。無一致の場合は入力そのままで、Packet/metaを保持する。本文が変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_regex_replace`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #21（管理Issue: #15）

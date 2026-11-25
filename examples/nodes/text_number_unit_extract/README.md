# 数値・単位抽出（実験）

`experimental.text-number-unit-extract@1` / `TextNumberUnitExtract`。管理Issue #38 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Text。出力 `out`: ja / Sequence。各項目は抽出された「数値+単位」表現の原文部分文字列で、位置は meta の source_span（コードポイント単位の半開区間）で表す。reading は付けない。元の順序を保つ。
- 完全設定の既定値: `{"limit":64}`。`limit` は件数上限（1〜1024）。未知キー・部分設定・不正型は invalid-config。
- 数値文法: 任意の符号（+ / -）、整数部、小数部からなる半角数字列。符号は直前が数字・小数点・符号でない場合だけ数値の一部とする。したがって範囲表記「1-2個」では「2個」だけが抽出される（決定的な規則として明記する）。全角数字・漢数字・桁区切りカンマは対象外。
- 単位は固定の内蔵表（SI記号と %・℃、円・人・個・歳・年・月・日・時・分・秒・時間などの助数詞）から最長一致で選ぶ。数値と単位の間の空白1つは表現に含める。
- ASCII記号単位は、直後が英数字なら単語の一部とみなして一致させない。単位のない数値・未知単位は抽出しない。
- 単位換算はしない（unit-notation とも独立）。空入力は空の Sequence。入力の meta は引き継がない。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_number_unit_extract`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #56（管理Issue: #38）

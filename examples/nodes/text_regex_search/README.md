# 正規表現検索（実験）

`experimental.text-regex-search@1` / `TextRegexSearch`。管理Issue #16 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。真偽判定は `experimental.text-compare` の責務であり、この Node は一致部分の抽出だけを行う。

- 入力 `in`: ja / Text。出力 `out`: ja / Sequence（一致ごとの Text 項目）。
- 完全設定の既定値: `{"pattern":"","limit":64}`。`limit` は一致件数の上限（1〜1024 の整数）。未知キー・部分設定・不正型は invalid-config。
- 正規表現エンジンと上限方針は text-regex-replace と共通（`regex` クレート、線形時間、pattern 1024 UTF-8 バイト以下、コンパイル済みサイズ各1MiBまで）。パターンの文法エラーやサイズ超過は invalid-pattern。
- 一致は左から右の順で返し、件数が limit に達したらその順序のまま打ち切る。ゼロ幅一致も含める。
- 各項目の位置は meta の `source_span`（コードポイント単位の半開区間）で表す。reading は付けない。入力 Packet の meta は引き継がない。キャプチャグループの展開は行わない（一致文字列のみ）。
- 無一致は空の Sequence で、NoOutput や空入力と区別する。空入力でもパターンが空に一致すれば一致として返す。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_regex_search`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #24（管理Issue: #16）

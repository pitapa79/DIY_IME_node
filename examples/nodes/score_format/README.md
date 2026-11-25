# スコア整形（実験）

`experimental.score-format@1` / `ScoreFormat`。管理Issue #95 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Choice。出力 `out`: ja / Sequence。候補と項目は位置で対応する。
- 完全設定の既定値: `{"decimals":2,"percent":false}`。decimals は小数桁数 0〜6、percent は100倍して % を付ける表示設定。未知キー・部分設定・不正な値は invalid-config。
- 各項目の Text は整形したスコア文字列。項目の meta の reading には、候補が Text の場合にその本文を入れる（候補が Text でない場合は meta なし）。候補自体の meta は引き継がない。
- score が未設定の候補は空文字列の項目になり、位置は保たれる。値そのものは変更しない（整形は表示だけ）。非有限の score を含む Packet は Core の Packet 検証で invalid-packet になり、整形には到達しない。
- percent は確率であるという主張ではなく、明示設定されたときだけの表示変換。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Choice以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_score_format`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #109（管理Issue: #95）

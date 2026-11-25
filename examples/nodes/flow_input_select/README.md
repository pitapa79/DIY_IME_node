# 入力選択（実験）

`experimental.flow-input-select@1` / `FlowInputSelect`。管理Issue #42 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b`: ともに任意（optional）の Packet ポート。根の型・言語は問わない。出力 `out`: Packet。
- 完全設定の既定値: `{"priority":"a-first"}`。`priority` は `a-first` / `b-first`。未知キー・部分設定・不正型は invalid-config。
- 優先側から順に、Data である最初の入力をそのまま（Packet・meta を含め同一のもの）出力する。空の Text や空の Sequence でも Data は Data として選ばれる。
- 選ばれた Packet だけを検証し、不正なら invalid-packet。選ばれなかった側は検証しない（不正でも影響しない）。
- 両方とも NoOutput または未接続なら out は NoOutput。missing-input は発生しない。

検査: `cargo test --locked --all-features --test nodes_flow_input_select`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #61（管理Issue: #42）

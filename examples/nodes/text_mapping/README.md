# 文字列対応表変換（実験）

`experimental.text-mapping@1` / `TextMapping`。管理Issue #17 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。ローマ字変換の overrides や単一リテラル置換（text-replace）とは別責務。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"entries":[],"unknown":"preserve"}`。`entries` は `{"from": ..., "to": ...}` の配列、`unknown` は `preserve`（未知部分を保持）/ `error`（拒否）。未知キー・部分設定・不正型は invalid-config。
- 変換は入力を左から右へ1回だけ走査する決定的な処理。各位置で entries の最長一致を選び、置換後の文字列は再走査しない（非再帰）。例: `a→b` と `b→c` があっても `a` は `b` になり、`c` にはならない。
- 検証: from の重複は invalid-config、空の from は invalid-config。空の to は削除として許可する。entries は最大256件、from は1〜64コードポイント、to は0〜64コードポイント。
- 未知部分: preserve は1コードポイントをそのまま写して進む。error は unmapped-input エラーにする。
- Packet検証後に処理する。NoOutputはNoOutput、空文字はData。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_mapping`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #20（管理Issue: #17）

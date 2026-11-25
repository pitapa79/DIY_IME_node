# スコア合成（実験）

`experimental.score-combine@1` / `ScoreCombine`。管理Issue #44 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b`: 必須、ja / Choice。出力 `out`: ja / Choice。
- 完全設定の既定値: `{"weightA":1.0,"weightB":1.0,"missing":"zero"}`。重みは有限数でなければならない（非有限は invalid-config）。未知キー・部分設定・不正型は invalid-config。
- 候補の同一性は meta を含む候補値の完全一致で判定する。まず入力ごとに重複候補の score を合計し（score の None は 0 として数える）、次に weightA・weightB を掛けて合計する。出力の score は常に合計値（Some）。
- 並びは a の出現順 → b にのみ現れる候補の出現順。
- `missing`: `zero` は片側だけの候補も 0 とみなして含める。`intersect` は両側に現れる候補だけを残す。
- 合計が非有限（無限大・NaN）になったら non-finite-score エラー。
- どちらか一方が NoOutput なら out は NoOutput。入力欠落は missing-input、Choice 以外は invalid-root、Packet が不正なら invalid-packet。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_score_combine`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #62（管理Issue: #44）

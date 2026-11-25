# 候補ランダム選択（実験）

`experimental.choice-random-select@1` / `ChoiceRandomSelect`。管理Issue #93 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Choice。出力 `out`: 選ばれた候補そのものを根とする Packet（根制約なし）。候補の meta は保持するが、候補の score は根へ移せず落ちる。
- 完全設定の既定値: `{"seed":0}`。seed は符号なし64ビット整数。未知キー・部分設定・不正な値は invalid-config。
- 選択は決定的: SplitMix64 を seed で初期化し、Lemire法（剰余の偏りを避ける乗算シフト）で一様に選ぶ。OS乱数・時刻・内部状態は使わないため、同じ seed と同じ候補列では常に同じ結果になる。
- 重み付き選択は初版の対象外（score は選択に影響しない）。
- 空の Choice は NoOutput。NoOutput入力はNoOutput。入力欠落はmissing-input、Choice以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_choice_random_select`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #107（管理Issue: #93）

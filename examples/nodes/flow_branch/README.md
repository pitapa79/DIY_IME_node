# 実行時分岐（実験）

`experimental.flow-branch@1` / `FlowBranch`。管理Issue #96 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `data`: 任意の Packet。入力 `selector`: Text。出力 `a` / `b`: 任意の Packet。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- selector の値が "a" / "b" のとき、data をそのままそのポートへ回し、他方は NoOutput。それ以外の値は invalid-selector。
- data が NoOutput、または selector が NoOutput のときは両出力とも NoOutput。どちらかの入力が欠ければ missing-input。selector が Text でなければ invalid-root。
- 重複確認（作業Issue #110 に記録）: demo.branch は設定で分岐先が固定され実行時には決まらない。text.compare / packet.compare はデータと比較条件が融合している。flow.input-select は入力の選択であり向きが逆。よって別kindとした。

検査: `cargo test --locked --all-features --test nodes_flow_branch`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #110（管理Issue: #96）

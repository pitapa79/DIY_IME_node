# 制約検証（実験）

`experimental.packet-validate-constraints@1` / `PacketValidateConstraints`。管理Issue #94 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: 任意の根の Packet。出力 `valid` / `invalid`: 条件を満たせば valid へ、満たさなければ invalid へ、元の Packet をそのまま回す。NoOutput入力では両出力とも NoOutput。
- 完全設定の既定値: `{"maxCandidates":null,"maxTextLength":null,"allowedRoots":[]}`。未知キー・部分設定・不正な値は invalid-config。
- 検査項目（null はその検査をしない）: maxCandidates は根が Choice のときの候補数の上限、maxTextLength は根が Text のときのコードポイント数の上限、allowedRoots は許す根の種類（空配列は全て許可、重複は invalid-config）。
- 初版は根だけを検査し、候補や項目の中までは降りない。
- このノードは Core の Packet::validate の代替ではない。不正な Packet は invalid-packet エラーであり、invalid 出力へは回さない。

検査: `cargo test --locked --all-features --test nodes_packet_validate_constraints`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #108（管理Issue: #94）

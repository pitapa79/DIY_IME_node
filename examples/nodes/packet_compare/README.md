# Packet比較（実験）

`experimental.packet-compare@1` / `PacketCompare`。管理Issue #43 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b`: 必須の Packet（根の型は問わない）。出力 `matched` / `unmatched`。
- 完全設定の既定値: `{"mode":"exact"}`。`mode` は `exact` / `content` / `structure`。未知キー・部分設定・不正型は invalid-config。
- 判定結果に応じて、a の元の Packet（meta を含め同一のもの）を matched 側または unmatched 側へ出し、他方は NoOutput にする。b は出力されない。
- exact: Packet 全体（版・根・meta・score を含む）の完全一致。
- content: すべての Text の meta を無視した内容一致。Choice の score は内容の一部として比較する。
- structure: 形だけの一致。根の種類、Sequence の長さ、Choice の候補数を再帰的に比較し、Text の中身・meta・score は見ない。
- どちらか一方が NoOutput なら両出力とも NoOutput。入力欠落は missing-input。Data 同士は両方を検証し、不正なら invalid-packet。

検査: `cargo test --locked --all-features --test nodes_packet_compare`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #60（管理Issue: #43）

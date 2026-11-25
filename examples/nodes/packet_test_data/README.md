# テストデータ生成（実験）

`experimental.packet-test-data@1` / `PacketTestData`。管理Issue #97 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `trigger`: 任意の Packet（内容は使わない。実行のきっかけとして明示の入力を必須とする）。出力 `out`: 生成した Packet。
- 完全設定の既定値: `{"shape":"text","size":16,"seed":0}`。shape は text / sequence / choice、size は 0〜1024、seed は符号なし64ビット整数。未知キー・部分設定・不正な値は invalid-config。
- 生成は決定的: SplitMix64 を seed で初期化し、固定のひらがな20文字から選ぶ。text は size 文字、sequence は3文字の Text を size 個、choice は3文字の候補を size 個（score は 0.000〜0.999 の決定的な有限値）。
- 生成した Packet は Packet::validate を通過する形だけを作る。実在の利用者入力や履歴は収集しない。
- trigger が NoOutput なら out も NoOutput。欠落は missing-input。不正な trigger Packet は invalid-packet。

検査: `cargo test --locked --all-features --test nodes_packet_test_data`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #111（管理Issue: #97）

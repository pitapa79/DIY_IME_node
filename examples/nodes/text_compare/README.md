# 文字列比較（実験）

`experimental.text-compare@1` / `TextCompare`。管理Issue #7 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b`: ともに必須、ja / Text。`a` を対象、`b` をパターンとする。
- 出力 `matched` / `unmatched`: ja / Text。現行のPacket契約には真理値型がないため、判定結果はこの2出力で表す。真なら `a` のPacketをそのまま（metaを含め変更せず）`matched` へ出し、`unmatched` はNoOutput。偽ならその逆。
- 完全設定の既定値: `{"mode":"exact"}`。`mode` は `exact`（完全一致）/ `contains`（aがbを含む）/ `prefix`（aがbで始まる）/ `suffix`（aがbで終わる）。未知キー・部分設定・不正型はinvalid-config。
- 比較は文字列の内容そのものに対するもので、大小文字を区別する。暗黙の正規化・大小変換は行わない（合成済みの「が」と分解形「か＋結合濁点」は `exact` で一致しない。正規化してから比較したい場合は text-unicode-normalize を前段に置く）。
- 空文字は通常の文字列規則どおり: 空同士は `exact` で真。空のパターンは `contains` / `prefix` / `suffix` で常に真。空の対象に空でないパターンは偽。
- 設定検証→入力欠落（missing-input）→Packet検証（invalid-packet）→root検査（invalid-root）の順に処理する。片方がNoOutputでも、もう片方の不正なDataは隠さない。どちらかがNoOutputなら全出力NoOutput。
- 入力を変更しない。入力の上限はPacket検証の上限に従い、このNode固有の上限は追加しない。

検査: `cargo test --locked --test nodes_text_compare`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #11（管理Issue: #7）

# Unicode正規化（実験）

`experimental.text-unicode-normalize@1` / `TextUnicodeNormalize`。管理Issue #6 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"form":"nfc"}`。`form` は `nfc` / `nfd` / `nfkc` / `nfkd` のいずれか。未知キー・部分設定・不正型はinvalid-config。
- 互換正規化（NFKC/NFKD）は明示的に選択した場合のみ行う。NFC/NFDでは半角カナや全角英字などの互換文字は変換しない。
- 正規化は `unicode-normalization` クレート（Unicode の正規化仕様に沿った表駆動の実装）に委ねる。自前の正規化表は持たない。
- Packet検証後に処理する。NoOutputはNoOutput、空文字はData。入力欠落はmissing-input、Text以外はinvalid-root。
- 本文が変わった時は入力metaを省略し、変わらない時はPacket/metaを保持する。入力を変更しない。
- 入力の上限はPacket検証の上限に従い、このNode固有の上限は追加しない。
- 正規化は冪等であり、同じ `form` を結果へ再適用しても本文は変わらない（検査で確認する）。

検査: `cargo test --locked --test nodes_text_unicode_normalize`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #9（管理Issue: #6）

# 文字数計算（実験）

`experimental.text-length@1` / `TextLength`。管理Issue #35 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。出力は長さを表す10進数の Text。
- 完全設定の既定値: `{"unit":"scalar"}`。`unit` は `scalar`（Unicode スカラー数）/ `byte`（UTF-8 バイト数）/ `grapheme`（書記素クラスタ数）。未知キー・部分設定・不正型は invalid-config。
- grapheme の数え上げは UAX #29 の正確な実装として `unicode-segmentation` クレート（MIT / Apache-2.0、純Rust）を optional 依存として opt-in-nodes feature に追加して使う。自前の近似実装は採らない。結合文字・ZWJ シーケンス・地域指示子・キーボードキャップ等は1クラスタになる。
- 出力は入力についての新しい事実なので、入力の meta は引き継がない（桁が偶然一致しても meta は付けない）。空文字は 0。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_length`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #53（管理Issue: #35）

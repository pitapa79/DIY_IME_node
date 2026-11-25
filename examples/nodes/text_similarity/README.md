# テキスト類似度（実験）

`experimental.text-similarity@1` / `TextSimilarity`。管理Issue #47 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b`: 必須、ja / Text。出力 `out`: ja / Text（類似度を表す10進数、meta は付けない）。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- 類似度は 1 − d / max(len_a, len_b)。d はコードポイント単位の Levenshtein 距離、長さもコードポイント単位。
- 値は小数第6位へ丸め、末尾の 0 は取り除く（1 は "1"、0.5 は "0.5"）。両方とも空なら "1"、片方だけ空なら "0"。
- 各入力は4096コードポイントまで。超えたら input-limit。
- どちらか一方が NoOutput なら out は NoOutput。入力欠落は missing-input、Text以外はinvalid-root、Packetが不正なら invalid-packet。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_similarity`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #65（管理Issue: #47）

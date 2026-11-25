# 数量表現変換（実験）

`experimental.text-quantity-format@1` / `TextQuantityFormat`。管理Issue #87 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- 入力は「アラビア数字（0〜9999）＋助数詞1文字」。出力は読みのひらがな表記（例: 3本→さんぼん）。出力の meta は付けない。
- 対応する助数詞は 本・個・人・枚・冊・匹・台・回 の8種のみ。数字の読みと音変化（促音化、ん の後の濁音化）はこのノード内で自己完結させる（#31 の未マージ実装には依存しない）。
- 人 は全体の例外として 1人=ひとり、2人=ふたり。4 を含む場合は よん→よ（4人=よにん、14人=じゅうよにん）。
- 形式に合わない入力（数字がない、助数詞が複数文字、10000以上）は invalid-quantity。表にない助数詞は unsupported-counter。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_quantity_format`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #101（管理Issue: #87）

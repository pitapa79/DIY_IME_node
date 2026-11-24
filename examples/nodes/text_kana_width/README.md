# カナ全角・半角変換（実験）

`experimental.text-kana-width@1` / `TextKanaWidth`。管理Issue #18 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。ASCII用の `experimental.ascii-width` は変更しない。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"mode":"fullwidth"}`。`mode` は `fullwidth`（半角→全角）/ `halfwidth`（全角→半角）。未知キー・部分設定・不正型は invalid-config。
- 変換表は JIS X 0201 の半角カナ（U+FF61〜U+FF9D）と全角カタカナの標準対応に基づく固定表。対象はカタカナ、長音、中点、半角句読点（｡｢｣､･）に限る。ひらがな・漢字など対象外の文字はそのまま保持する（NFKC のような広範な互換変換はしない）。
- 濁点・半濁点: 全角→半角では合成済みカナを基底カナ+濁点/半濁点マーク（U+FF9E / U+FF9F）へ分解する（ガ→ｶﾞ、パ→ﾊﾟ、ヴ→ｳﾞ、ヷ→ﾜﾞ、ヺ→ｦﾞを含む）。半角→全角では合成可能な並びを合成し、合成できないマークは結合濁点（U+3099）/ 結合半濁点（U+309A）として残す（ｱﾞ→ア+U+3099）。単独の濁点（U+309B）/ 半濁点（U+309C）と結合形は半角方向で U+FF9E / U+FF9F になる。
- 半角を持たない全角カナ（ヰ・ヱ・ヵ・ヶ・ヮ・ヸ・ヹ）は halfwidth 方向でも保持する。したがって可逆なのは半角カナと対応のある範囲に限られ、それ以外は往復で元に戻らない。
- Packet検証後に処理する。NoOutputはNoOutput、空文字はData。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_kana_width`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #23（管理Issue: #18）

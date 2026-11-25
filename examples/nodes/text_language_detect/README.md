# 言語判定（実験）

`experimental.text-language-detect@1` / `TextLanguageDetect`。管理Issue #90 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: Text（言語制約なし）。英語も受け付けるため、ポートに言語制約は付けない（事前確認を作業Issue #104 に記録）。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- 出力は `ja` / `en` / `und` のいずれかの Text（meta なし）。
- 判定規則（文字種の有無のみで決める決定的ヒューリスティクス）: ひらがな・カタカナがあれば ja。漢字がなくラテン文字だけなら en。それ以外（漢字のみ、漢字＋ラテン文字、数字・記号のみ、空）は und。
- 限界: 統計モデルではなく誤判定し得る。漢字のみの中国語と日本語は区別できない。対象は ja/en/und の3値のみ。
- 文字種の数え上げはこのノード内で自己完結させる（#36 の未マージ実装には依存しない）。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_language_detect`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #104（管理Issue: #90）

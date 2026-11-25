# あいまい辞書検索（実験）

`experimental.text-dictionary-fuzzy@1` / `DictionaryFuzzy`。管理Issue #41 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- prepare で辞書TSV（text.dictionary-exact と同じ書式・全件検証）を読み、不変の辞書を所有する。未prepareの実行は dictionary-not-prepared。
- 入力 `in`: ja / Text。出力 `out`: ja / Choice（候補 Text の meta は常に省略、score はTSVの重み）。
- 完全設定の既定値: `{"path":"dictionary.tsv","limit":64,"maxDistance":1}`。`limit` は候補数上限（1〜1024）、`maxDistance` は編集距離の上限（0〜3）。未知キー・部分設定・不正型は invalid-config。
- 距離は読み同士のコードポイント単位の Levenshtein 距離。長さ差による打ち切りと帯状DP（行最小値による早期終了）で計算する。
- 候補の順序: 距離の昇順 → 読みの昇順 → 同一読み内は元の行順。距離そのものは候補に付けず、順序でのみ表す。maxDistance=0 は完全一致のみ。
- 空入力・一致なしは空の Choice（原文フォールバックはしない）。索引成果物v1は読みのハッシュ順であり距離探索に使えないため再利用しない。
- 入力が4096コードポイントを超えると dictionary-input-limit。Packet検証・資源エラーコードは dictionary-prefix と同じ。NoOutputはNoOutput、入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_dictionary_fuzzy`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #57（管理Issue: #41）

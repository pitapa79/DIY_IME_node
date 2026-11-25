# 辞書分かち書き（実験）

`experimental.text-dictionary-segment@1` / `DictionarySegment`。管理Issue #39 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- prepare で辞書TSV（text.dictionary-exact と同じ書式・全件検証）を読み、不変の辞書を所有する。未prepareの実行は dictionary-not-prepared。
- 入力 `in`: ja / Text（読み文字列）。出力 `out`: ja / Choice。各候補は分割された読み部分文字列の Sequence（meta なし、score なし）。表層は含めない。
- 完全設定の既定値: `{"path":"dictionary.tsv","limit":64}`。`limit` は候補数上限（1〜1024）。
- 分割の規則: 入力全体を区間で覆う。各区間は辞書の読み（既知）か未知区間。連続する未知文字は1区間にまとめ、未知区間同士は隣接しない。区間が辞書にあるかどうかは文字列だけで決まる（同じ分割が二重に現れない）。
- 候補の順序: 未知文字数が少ない順 → 区間数が少ない順 → 区間列の辞書式順序。列挙は最良優先で決定的に行い、内部の展開上限（100,000回）で打ち切ることがある。
- 空入力は空の Choice。辞書に一切一致しない入力は、全体を1つの未知区間とする候補1件だけを返す。
- 索引成果物v1は読みのハッシュ順であり分割に使えないため再利用せず、検証済みTSVから読み集合を直接使う。入力が4096コードポイントを超えると dictionary-input-limit。Packet検証・資源エラーコードは dictionary-prefix と同じ。
- NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_dictionary_segment`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #59（管理Issue: #39）

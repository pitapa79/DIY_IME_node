# 辞書逆引き（実験）

`experimental.text-dictionary-reverse@1` / `DictionaryReverse`。管理Issue #19 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: 必須、ja / Text（表記）。出力 `out`: ja / Choice（候補は読み Text、score に辞書の重み）。
- prepare で明示Hostルートから辞書TSVを取得し、`text.dictionary-exact` と同じ全件検証（UTF-8、ヘッダー、重複、上限）を通過した不変の辞書を所有する。検証と収集は dictionary_exact の共通部品を共有し、複製しない。辞書の破損は prepare で同じエラーコード（dictionary-format 等）のまま失敗する。
- 索引成果物 v1（producer / consumer）は再利用しない。エントリが読みのハッシュ順に並び、表記からは引けないためである。逆引き構造は prepare 時に検証済みデータから構築する。
- 設定は `{"path":"dictionary.tsv","limit":64}` の完全なオブジェクト。path の規則は dictionary-exact と同じ。limit は総候補数の上限（1〜1024 の整数）。未知キー・部分設定・不正型は invalid-config。
- 結果の順序は元の行順で決定的にし、重みで並べ替えない（同点順序は行順）。重みは score としてそのまま載せる。候補の順位付けは候補選択 Node の責務。候補 Text の meta は常に省略する。
- 完全一致・前方一致との差: 一致なしは空の Choice（原文を候補として返すフォールバックはしない）。空入力も空の Choice。
- NoOutput は NoOutput。入力欠落は missing-input、Text 以外は invalid-root、4096 コードポイント超の入力は dictionary-input-limit、不正 Packet は invalid-packet。未準備の execute は dictionary-not-prepared。
- 入力を変更しない。資源契約・権限は dictionary-exact と同じで、この Node 固有の追加はない。取消は既存の prepare / 資源機構の性質を引き継ぐ。

検査: `cargo test --locked --all-features --test nodes_dictionary_reverse`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #22（管理Issue: #19）

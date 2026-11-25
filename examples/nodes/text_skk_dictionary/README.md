# SKK辞書検索（実験）

`experimental.text-skk-dictionary@1` / `TextSkkDictionary`。管理Issue #92 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Text。出力 `out`: ja / Choice（候補は Text、score は付けない）。
- 完全設定の既定値: `{"path":"dictionary.skk","limit":64}`。path は明示Host資源名、limit は 1〜1024。未知キー・部分設定・不正な値は invalid-config。
- prepare で辞書を読み、全件検証して不変の辞書として所有する。通常の execute は dictionary-not-prepared。
- SKK形式の解釈: `;` で始まる行はコメントとして読み飛ばす。項目行は「見出し /候補1/候補2;注釈/…/」で、候補の注釈（;以降）は取り除く。送りあり見出し（末尾のローマ字）も通常の見出しとして扱う。同じ見出しが複数行あれば候補をファイル順に連結する。形式に合わない行は dictionary-format。
- 初版は UTF-8 のみ。UTF-8 でない辞書は prepare 時に dictionary-read。辞書への書き込みと IME 状態は対象外。
- 検索は見出しの完全一致のみ。一致なし・空入力では空の Choice を返す。入力が4096コードポイントを超えると dictionary-input-limit。
- 資源エラーの対応は既存の辞書ノードと同じ（resource-path 系→dictionary-path、resource-limit→dictionary-limit、読み取り系→dictionary-read）。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_skk_dictionary`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #106（管理Issue: #92）

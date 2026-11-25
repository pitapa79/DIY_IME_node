# ひらがな→ローマ字変換（実験）

`experimental.text-hiragana-to-romaji@1` / `TextHiraganaToRomaji`。管理Issue #30 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。既存の `text.romaji-to-hiragana`（逆方向）は変更せず、可逆性は保証しない。

- 入力 `in`: ja / Text。出力 `out`: ja-Latn / Text。
- 完全設定の既定値: `{"system":"hepburn"}`。初版はヘボン式のみ。未知キー・部分設定・不正型は invalid-config。
- 変換表は音節単位の固定表（拗音は1音節として変換）。表にない組合せ（ふぁ等）は文字ごとに変換する。ひらがな以外の文字（カタカナ・漢字・英数字・記号・長音記号 ー）は変換せず保持する。
- 促音（っ）: 後続音節の頭子音を重ねる（がっこう→gakkou）。ただし ch の前は t を重ねる（まっちゃ→matcha）。母音・ん・文末・非ひらがなの前では `'` とする（あっ→a'）。
- 撥音（ん）: 原則 n。母音と y の前では n' とし、読みの区切りを曖昧にしない（しんよう→shin'you、かんい→kan'i）。
- 長音は母音をそのまま書く（おう→ou、えい→ei）。マクロン（ō 等）は使わない。
- 小書き母音 ぁぃぅぇぉ は a/i/u/e/o、ゎ は wa、ゐ/ゑ は wi/we。拗音にならない位置の小書き ゃゅょ は単独で ya/yu/yo とする。
- Packet検証後に処理する。NoOutputはNoOutput、空文字はData。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_hiragana_to_romaji`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #50（管理Issue: #30）

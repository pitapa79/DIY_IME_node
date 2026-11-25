# 異体字候補生成（実験）

`experimental.text-kanji-variants@1` / `TextKanjiVariants`。管理Issue #86 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in`: ja / Text。出力 `out`: ja / Choice（候補は Text、score は付けない）。
- 設定はなし（空オブジェクト `{}` のみ有効）。未知キーは invalid-config。
- 入力がちょうど1文字で対応表にある場合、先頭に元の字、続いて表の順序で異体字を候補として返す。それ以外（複数文字、表にない文字、空文字列）は入力そのものだけを含む Choice を返す。
- 対応表はコード内の明示データ（10組）で、字形の対応という事実関係を自前で編集したもの。版は本PR時点。#85 の旧新字体表とは独立に持ち、必須依存にしない。
- 異体字セレクタ（IVS）付き入力は複数コードポイントになるため対象外で、単一候補としてそのまま返す。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_kanji_variants`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #100（管理Issue: #86）

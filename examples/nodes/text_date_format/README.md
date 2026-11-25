# 日付表記変換（実験）

`experimental.text-date-format@1` / `TextDateFormat`。管理Issue #32 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。入力 Text 全体が明示形式の日付である場合だけ変換する。
- 完全設定: `{"from": ..., "to": ...}`。形式は `iso`（YYYY-MM-DD、月日とも2桁ゼロ埋め必須）/ `slash`（YYYY/M/D、月日は1〜2桁）/ `japanese`（YYYY年M月D日）。年は全形式とも4桁。年を省略した入力は受け付けない。未知キー・部分設定・不正型は invalid-config。既定値は `{"from":"iso","to":"japanese"}`。
- 検証は proleptic Gregorian 暦で行い、うるう年（400年規則を含む）を含む実在日だけを通す。年は 1〜9999。存在しない日付・形式違いは invalid-date エラーとし、月日順の推測はしない。空入力も invalid-date。
- 出力は iso = YYYY-MM-DD、slash = YYYY/MM/DD（どちらもゼロ埋め）、japanese = YYYY年M月D日（埋めなし）に統一する。
- 現在日時・OS時刻の取得はしない。
- Packet検証後に処理する。NoOutputはNoOutput。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_date_format`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #48（管理Issue: #32）

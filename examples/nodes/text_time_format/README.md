# 時刻表記変換（実験）

`experimental.text-time-format@1` / `TextTimeFormat`。管理Issue #33 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。入力 Text 全体が明示形式の時刻である場合だけ変換する。
- 完全設定: `{"from": ..., "to": ...}`。形式は `h24`（H:MM[:SS]）/ `h12`（H:MM[:SS] AM|PM）/ `japanese`（午前・午後H時M分[S秒]）。未知キー・部分設定・不正型は invalid-config。既定値は `{"from":"h24","to":"japanese"}`。
- 秒の有無は入力に合わせ、全形式で保持する。出力の分・秒は2桁ゼロ埋め。h24 の時は2桁ゼロ埋め、h12 と japanese の時は埋めなし。
- 12時間制の時は 1〜12（午前0時・午後13時は invalid-time）。0時は 午前12時、12時は 午後12時に対応する。
- 24時の扱い: 24:00（秒がある場合は 24:00:00）だけを有効とし、h24 出力ではそのまま保持する。他形式への変換は invalid-time とする（翌日0時の意味を勝手に確定しない）。24:01 等は常に無効。60分・60秒もうるう秒を含め無効。
- 時計取得・時差変換はしない。無効値は推測で確定せず invalid-time エラー。空入力も invalid-time。
- Packet検証後に処理する。NoOutputはNoOutput。変換が起きなかった場合は入力そのままで Packet/meta を保持し、変わった時は入力metaを省略する。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_time_format`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #51（管理Issue: #33）

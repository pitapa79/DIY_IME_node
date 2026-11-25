# 漢字字体変換（実験）

`experimental.text-kanji-form@1` / `TextKanjiForm`。管理Issue #85 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。
- 完全設定の既定値: `{"direction":"to-shinjitai"}`。direction は `to-shinjitai`（旧字体→新字体）または `to-kyujitai`（逆方向）。未知キー・部分設定・不正な値は invalid-config。
- 変換はコード内の明示対応表（241組）だけで行う。一般的な Unicode 正規化とは別機能で、正規化は使わない。
- 対応表は旧字体・新字体の対応という文字の事実関係を自前で編集したもので、版は本PR時点。表にない文字はそのまま保持する。
- 曖昧なグループ（複数の旧字体が1つの新字体に対応）: 弁 ← 辨・瓣・辯、辺 ← 邊・邉。to-shinjitai では全て新字体へ寄せる。to-kyujitai では代表形（弁→辨、辺→邊）へ変換し、意味による使い分けはしない。この扱いは仕様として明記するもので、逆変換が一意であるとは主張しない。
- 変換で変化しない場合は入力Packetをそのまま返す（meta保持）。変化した場合は新しい Text（meta なし）。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。

検査: `cargo test --locked --all-features --test nodes_text_kanji_form`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #99（管理Issue: #85）

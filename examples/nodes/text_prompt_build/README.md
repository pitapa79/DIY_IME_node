# プロンプト構築（実験）

`experimental.text-prompt-build@1` / `TextPromptBuild`。管理Issue #46 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `in` / 出力 `out`: ja / Text。出力は instruction + before + 入力 + after の連結で、meta は付けない。
- 完全設定の既定値: `{"instruction":"","before":"\n\n","after":""}`。instruction は最大4096 UTF-8バイト、before / after は各256 UTF-8バイトまで（超過は invalid-config）。未知キー・部分設定・不正型は invalid-config。
- 入力はリテラルに挿入する（プレースホルダ展開や再走査はしない）。連結は常に4要素の単純連結であり、instruction が空でも before はそのまま付く。
- 出力が 1 MiB を超えたら output-limit。
- text-concat（対等な入力同士の連結）や text-snippet-expand（複数値の汎用テンプレート）とは別責務として、固定指示文＋囲み＋単一入力の形を専用 kind にした（作業Issue #63 に事前確認の記録あり）。
- Packet検証後に処理する。NoOutputはNoOutput。入力欠落はmissing-input、Text以外はinvalid-root。入力を変更しない。

検査: `cargo test --locked --all-features --test nodes_text_prompt_build`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #63（管理Issue: #46）

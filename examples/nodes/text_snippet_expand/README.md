# スニペット展開（実験）

`experimental.text-snippet-expand@1` / `TextSnippetExpand`。管理Issue #45 の仕様案に基づく実験実装。正式採用・標準同梱は未決定で、この実装をもって昇格しない。

- 入力 `a` / `b` / `c` / `d`: 任意（optional）の ja / Text。出力 `out`: ja / Text（meta は付けない）。
- 完全設定の既定値: `{"template":"{a}"}`。template は最大4096 UTF-8バイト。未知キー・部分設定・不正型は invalid-config。
- プレースホルダは `{a}`〜`{d}` のみ。`{{` は `{`、`}}` は `}` のリテラル。宣言されていないプレースホルダ・閉じていない `{`・単独の `}` は validate_config の時点で invalid-config。
- テンプレートが使うポートが入力マップに無い場合は missing-input。使うポートが NoOutput なら out は NoOutput（伝播）。使わないポートは状態によらず無視する。
- なお実行エンジンは未接続の任意ポートを NoOutput として届けるため、エンジン経由では「使うポートの未接続」は NoOutput 伝播として現れる。missing-input は入力マップ自体にポートが無い直接実行でのみ区別できる。
- 使うポートの Packet だけを検証する（invalid-packet / invalid-root）。挿入した値は再走査しない（値に含まれる `{b}` などは展開されない）。
- 展開結果が 1 MiB を超えたら output-limit。

検査: `cargo test --locked --all-features --test nodes_text_snippet_expand`。正式fixtureを弱めず、このディレクトリーの契約例を実装に照合する。実Wasm検証とWindows CIの結果はPR側で確認する。

戻し方: 対象PRをrevertする。実験kindを使用したグラフは、別ノードへ交換してから実装を除去する。

対象Issue: #64（管理Issue: #45）

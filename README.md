# DIY_IME_node

DIY_IMEのプリミティブ・標準機能・実験Nodeのソースを保守するリポジトリです。公開準備中です。現在の本体は [DIY_IME](https://github.com/pitapa79/DIY_IME) です。コピー元と省略した過去資料は [移行記録](docs/migration.md) を参照してください。

## 開発運用ルール

**[Node開発・運用規則 v1.0](docs/node-development-governance.md)** を正本とします。1ノード1管理Issue、変更ごとの短期Issue/ブランチ/PR、AIの提案とユーザーによる実装・採用承認の分離、技術マージの条件などを定めています。Codex・Claude等は作業開始時に [AGENTS.md](AGENTS.md) と担当Issueも確認してください。旧Issue/PRを遡及変更せず、既存ノードの管理Issueは必要に応じて段階的に作成します。
[本体](https://github.com/pitapa79/DIY_IME) / [移行統括 #257](https://github.com/pitapa79/DIY_IME2/issues/257)。

## 現在の状態

50個の非システムNodeを移行しました。既存48個をregister_nodesで登録し、段落分割・復旧の2個は未登録prototypeのままです。
Input/Output等のシステムNodeは本体が所有します。tests/supportの4システム実装は検査用fixtureであり、配布Nodeではありません。
本体側は実Wasm隔離・GUIインポート・個別承認・辞書/通信/秘密管理に加え、公式同梱・既定Wasm切替・Windows配布版の基本結合検証まで完了しています（[本体 #257](https://github.com/pitapa79/DIY_IME2/issues/257)）。実TSF統合と実用IMEの応答時間目標は未達です。RustのNode traitを公開外部ABIとして扱わないでください。

## 開発と検査

Rust1.90 GNU/Windowsを基準にしています。通常のNode修正は個別テスト、統合時は次を実行します。

~~~text
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
pwsh -File scripts/check-migration.ps1
~~~

Nodeリポジトリ単体の検査に本体のcloneやGitHub取得認証は不要です。sdk/runtimeは本体の固定コミットから取得した開発用Core snapshotで、SDKソースをNode担当が直接変更しないでください。
migration-provenance.jsonに元commit/各blobと移行後blobを記録しています。定義比較は元の54件台帳から非システム50件を照合します。
CLIのdiy-ime2 binaryはtests/support/cli.rsの検査用Hostです。アプリの配布用CLIではありません。

個別Nodeはsrc/nodes、互換demo/e2eの処理はsrc/compatにあります。正式fixtureの期待値を実装都合で弱めないでください。
変換結果/登録kind/version/schema/defaultを維持し、分類と正式採用/実装状態を混同しません。
本体の旧処理コピーは、Wasm経路の切替・互換性検証後に除去済みです（[本体 #257](https://github.com/pitapa79/DIY_IME2/issues/257)）。

実WasmのAppContainer/Job検査手順と限界は[隔離CI](docs/isolated-wasm-ci.md)を参照してください。本体側の接続状況は[移行統括](https://github.com/pitapa79/DIY_IME2/issues/257)、独自通信ノードの追加APIは[Service guest SDK](docs/service-guest-sdk.md)を参照してください。

公式48パッケージの生成・検証・source receiptは[配布データ生成](docs/official-node-packages.md)を参照してください。本体への同梱・既定切替は完了済みです。

## 利用条件

プロジェクト全体の利用許諾は未決定です。公開設定と利用・改変・再配布の許諾は別に扱います。依存コンポーネントにはそれぞれの利用条件が適用されます。

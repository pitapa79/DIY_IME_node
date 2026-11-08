# DIY_IME_node へのコピー記録

2026-11-08。開発元は `pitapa79/DIY_IME2_node`、基準mainは `763f1af4bfecb86ca725d9d12f7d626bcc9e51bd`。本コピーは新しい履歴で管理する。ファイルごとのコピー元blobとコピー後SHA-256は `publication-snapshot.json` に記録する。

## 収録範囲

現在のソース、固定SDK、正式fixture、登録・定義台帳、ビルドと検査script、現在の契約文書を収録する。`docs/legacy-node-inventory.json` は登録互換性テストと配布物の検証で利用するため保持する。

`examples/nodes/ascii_width/HANDOFF.md` は過去の担当引継ぎ・使用量・ローカルパスを含む作業記録のためスキップした。原本は非公開の移行元に残す。対応する移行台帳の1エントリーだけを除外し、残る245件、固定SDKと正式fixtureの元commit/blobの検査を維持する。旧Git履歴、過去のIssue/PR、Actionsログとartifactは本コピーへ収録しない。

## 名称と来歴

現在の案内は本体 `pitapa79/DIY_IME`、Node側 `pitapa79/DIY_IME_node` を指す。歴史的な採用・元commitへのリンクは過去の根拠として残る。新しく生成する配布indexとbuild receiptは新Nodeリポジトリを記録する。SDK、契約の出自と、本体で既に配布されている旧成果物の来歴は元の記録を維持する。

## 保留中の開発

旧PR #20（`8d9d2a0d42c1647f0e592778053bab8254ba345a`）と #26（`35ce3844d54e87e1577ad88850ac43802dd525d0`）は今回のmainコピーへ含めていない。引継ぎ時は旧Issue #19、#24、#25と承認・依存を確認し、新しい作業Issue/PRで検査する。

## 公開準備とCI

ユーザーの指示により、初回push前に移行先リポジトリのActionsを無効化した。公開準備が完了するまで無効状態を維持する。workflowファイルは既存の検査を実行できる状態で収録する。ローカル検査とGitHub CIの成功は個別に記録する。

調査・作業記録は [Issue #1](https://github.com/pitapa79/DIY_IME_node/issues/1)。公開設定、CI再開、未統合PRの取込みは進捗を確認してから扱う。

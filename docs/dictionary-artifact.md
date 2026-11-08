# 辞書の事前準備と範囲検索

`text.dictionary-exact@1` の別経路を構築中。A/B2の仕様採用を受けた実装だが、通常Hostへの切替・既定同梱・保持Workerはこの変更に含まない。従来の48登録・設定・正式fixture・既定v1 moduleを維持する。

## 準備と検索

producer は Host が渡した原本の不変スナップショットを SDK の範囲読取で取得する。既存 `ExactDictionary::from_tsv_bytes` による全件検証が成功してから、Node固有形式 `dictionary-exact-index@1` を構築・出力する。末尾不正・重複・既存上限違反があれば成果を出さない。Host は正常な実終了と完全な出力を確認してから成果を採用する必要がある。

consumer は同じ設定pathの成果を読み、読みのhashと全文の組で二分探索する。hashはUnicode scalarの多項式31（wrapping u64）であり、セキュリティ用ではない。衝突時も全文を照合するので、衝突数に比例する走査にならない。行順・有限f64 score・meta省略・未知語・空入力・NoOutputは従来処理と同じ。候補全件の読取と検証が終わるまで成功を返さず、資源エラーを未知語に置き換えない。

バイナリは32 byte header（magic `DIYDIC01`、版、行数、読み数、path byte数、総byte数）、path UTF-8、読みごとの32 byte索引、読みUTF-8と候補ブロックで構成する。数値はlittle endian。索引はhash u64、読みoffset u64/length u32、候補offset u64/length u32。候補ブロックは件数u32と、surface length u32/UTF-8/score f64の列。読みごとにhash・全文の順に整列する。

原本の1MiB/10,000行/行64KiB/4096コードポイント/64候補上限は変えない。成果は別枠の最大4MiB。一度の範囲読取は64KiB以内。巨大候補も複数回で全件取得し、切り捨てない。4096文字級の候補について性能改善は後回しにし、機能互換性を検査する。4MiBは原本制限と索引・各行の有限オーバーヘッドを収容する保守的上限で、プロセスRSS保証ではない。

任意の外部blobをこの形式名だけで信頼してはならない。Hostは検証済producer、原本SHA、設定、Node定義、成果SHAを結び付けた準備成果だけを渡す。consumerの読取範囲・ヘッダ・UTF-8・候補検査は、入力ごとの全件再検証を行うものではない。Hostの取消・文書/セッション境界・期限・実終了/物理会計は本体の責務。

## 固定SDKとビルド

`sdk/wasm-resource-sdk` は公開本体 [PR #69](https://github.com/pitapa79/DIY_IME/pull/69) の `1aa3a9400eed9d8b5ba9f806497d6f97acd87430` からbyte単位で固定した当時の候補snapshot。由来はその時点の固定コミットで、現在の本体 main のバイトを指さない。旧 `sdk/runtime` は変更しない。独立receiptと `scripts/check-resource-sdk.ps1` で4ファイルと出自を検査する。新しい第三者依存バージョンはない。

```powershell
./scripts/check-resource-sdk.ps1
cargo test --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
./scripts/build-dictionary-artifact.ps1
```

生成先はCargo target内の `dictionary-artifact/{producer,consumer}.wasm` と `build-receipt.json`。producerだけがemit importを使うため別moduleとし、専用targetで既定Wasmの上書きを防ぐ。既定moduleの前後比較は同じ環境・条件で行う。本体に既に同梱されたWasmのhashとの一致は、この比較だけでは主張しない。

移行台帳の辞書mod blobは条件付きmodule宣言追記、Cargo.lock blobはpath依存2パッケージ追加に合わせて更新する。元sourceBlob/sourceCommitと正式fixtureは保持する。

## 残る検証と統合

実Native brokerでの検査は `scripts/check-dictionary-artifact.ps1` から行う。認証不要の公開本体archiveを commit `9dbda8dfad8660226272b268a7f8956f6baf9b1c` / ZIP SHA256 `febf6613a706fbdbdc90a504ac10986bf6031049679487c18a2e20cdc1712145` に固定し、開発専用probeをビルドする。[本体PR #71](https://github.com/pitapa79/DIY_IME/pull/71) の当時の候補スナップショットであり、現在の製品 main の検証とは区別する。既存probeを明示指定するローカル実行も可能で、そのoverrideをreceiptに記録する。非公開Nodeソースを公開本体へ持ち込まない。

公開 Node 側の実検査では、正式例、開始前取消、読みの衝突、空辞書、不正原本の拒否、4096文字×64候補の全件取得と OS 実終了を確認する。巨大候補は検査用 fuel 5B を使用し、通常条件での性能判定は行わない。

通常Hostでの辞書切替、固定公式構成の選択、保持Worker、実用辞書の全体速度測定は未完了。公開 Node 側の CI 結果と本体側の採用・性能検査は別に記録する。

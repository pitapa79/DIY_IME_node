# 実WasmのAppContainer/Job回帰検査

本体[Worker probe](https://github.com/pitapa79/DIY_IME2/pull/267)を固定して、
開発用にdev/appcontainer-worker-probe.rs とdev/security_support/windows_worker.rsへ取り込む。
元commit/path/blobはdev/worker-probe-origin.json、照合はscripts/check-worker-probe.ps1。
Core SDK、Node実装、正式fixtureはこのCI工程では変更しない。
WAT拒否fixtureも元拡張子を保持して同じWasmtime解析経路を使用する。

~~~powershell
./scripts/check-wasm-runtime.ps1 -Isolated
~~~

既存scripts/check-wasm-guest.ps1の54正常・構造化拒否、6不正Wasm拒否、正常回復を
scripts/invoke-isolated-probe.ps1のアダプター経由で再利用する。元の期待値・拒否理由照合は変えない。
Parentが確認した真正終了とprofile削除を検査してから、既存の比較へresultだけを渡す。
Workerの実行exeは所有する検査scriptがCargo metadataから選び、検査の間だけ明示する。
隔離起動失敗を意図したWasm拒否と誤認しないよう、6種類それぞれの固有理由も照合する。
非隔離経由の検査と隔離経由の検査をCIで両方実行する。片方の失敗を他方で置き換えない。

AppContainerはcapabilitiesなし、起動時Job割当、resume前に実AppContainer token/SID/Job所属を照合する。
任意の親環境・handleは継承しない。専用cacheにread/executeだけを付与する。
Native偽canaryの通常権限対照と拒否、100ms timeoutの真正終了と新Job回復も実行する。
検査profileは終了後に削除する。生成cacheはtarget配下に調査用として残る。
windows-sysはWindows開発用の依存だけで、wasm32 guestへOS APIは渡さない。

これは開発用の境界検査。GUI import、production Worker IPC/取消/実行slot、
権限broker、保護ゲート、辞書/network/secret broker、公式配布切替は後続。
AppContainer内の全OS資源が不可視という保証や、全インターネット経路の検査ではない。
SDK/APIの公開安定版や外部Native package許可を意味しない。
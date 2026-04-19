# Windows 11 省エネ機能状態取得メモ

## 目的

Windows 11 の「現在の電源モード」と「省エネ機能の有効状態」を正確に区別して扱うための調査結果と設計判断をまとめる。

このドキュメントの主眼は次の 2 点である。

- `Best Power Efficiency` と `Engery saver (省エネ機能)` を混同しないこと
- Windows 11 24H2 系の `Energy saver` まで含めて、実用上破綻しない判定方法を残すこと

## 結論

### 推奨方針

Windows 11 の省エネ機能状態を取りたい場合、一次情報として使うべきなのは `GUID_ENERGY_SAVER_STATUS` に対する `PowerSettingRegisterNotification` である。

この組み合わせを使うと、以下を満たせる。

- `Best Power Efficiency` とは独立に `Energy saver` の ON/OFF を扱える
- 登録直後に現在値が callback で返るため、初期状態取得と変更追従を同じ仕組みで処理できる
- `STANDARD` と `HIGH_SAVINGS` の両方を拾える

### 補助方針

`GetSystemPowerStatus().SystemStatusFlag` は fallback としては使えるが、Windows 11 24H2 の `Energy saver` 判定の主経路には使わない方がよい。

理由:

- これは従来の `Battery saver` 系の状態取得としては有効
- しかし今回の確認では、新しい `Energy saver` を正しく反映しないケースがあった

### 非推奨方針

`PowerRegisterForEffectivePowerModeNotifications` の `EFFECTIVE_POWER_MODE` を、省エネ機能そのものの ON/OFF 判定に使うべきではない。

理由:

- これは「省電力の動作モード」を表す API であり、省エネ機能(Energy saver)トグルOn/Offそのものではない
- `Best Power Efficiency` と `Energy saver` の区別がつかず誤判定を招く

## 背景

このアプリは、タスクトレイのメニューから Windows 11 の power mode overlay を切り替える。

対象となる power mode は次の 3 つである。

- `Balanced`
- `Best Performance`
- `Best Power Efficiency`

ここに別軸で `省エネ機能 (Energy saver)` が存在する。

重要なのは、`Best Power Efficiency` と `省エネ機能 ON` は同じものではないという点である。

- `Best Power Efficiency` は power mode overlay の一種
- `省エネ機能 ON` は OS の追加的な節電状態

そのため、メニューの無効化条件を決めるときに両者を混同すると、次のどちらかが起こる。

- `Best Power Efficiency` にしただけでメニューが無効化される
- 本当に `省エネ機能 ON` なのにメニューが有効なままになる

## 今回確認した API ごとの性質

## 1. overlay GUID API

対象 API:

- `PowerGetEffectiveOverlayScheme`
- `PowerGetActualOverlayScheme`
- `PowerSetActiveOverlayScheme`

用途:

- 現在の power mode overlay を取得する
- power mode overlay を設定する

向いている用途:

- `Balanced / Best Performance / Best Power Efficiency` の判定
- トレイアイコンの表示切替
- メニューからの power mode 切替

向いていない用途:

- `省エネ機能 ON/OFF` の判定

理由:

- overlay GUID には `Energy saver` の状態そのものは出てこない
- `Best Power Efficiency` と `省エネ機能 ON` は別概念だから

## 2. `PowerRegisterForEffectivePowerModeNotifications`

対象 API:

- `PowerRegisterForEffectivePowerModeNotifications`
- `PowerUnregisterFromEffectivePowerModeNotifications`

用途:

- システムの実効的な power mode の通知を受ける

一見すると使えそうに見える理由:

- `EFFECTIVE_POWER_MODE` に `BatterySaver` や `BetterBattery` 風の値がある
- 省電力寄りの状態を通知してくれる

しかし主用途には不適切だった理由:

- これは `Energy saver` トグルの状態 API ではない
- `Best Power Efficiency` を選んだだけでも、省電力寄りの effective mode が返ることがある
- そのため「省エネ機能が ON か」を直接判断するには意味がずれる

今回の不具合:

- `Best Power Efficiency` に切り替える
- effective mode 側では省電力寄りの値が返る
- それを `Energy saver` と誤認し、メニューが無効化される

補足:

- SDK header 上の `EFFECTIVE_POWER_MODE` 定義は、以前想定していた alias 付きの並びではなく、実際には `BatterySaver -> BetterBattery -> Balanced -> HighPerformance -> MaxPerformance ...` の形だった
- ただしこの解釈を正しただけでは、`Energy saver` 判定の根本問題は解決しない
- 問題は enum 値の読み違いだけでなく、API の意味自体が判定目的と一致していなかったことにある

## 3. `GetSystemPowerStatus().SystemStatusFlag`

対象 API:

- `GetSystemPowerStatus`
- `SYSTEM_POWER_STATUS.SystemStatusFlag`

用途:

- 従来の `Battery saver` 状態取得

期待した理由:

- Microsoft Docs 上でも `SystemStatusFlag` は battery saver 状態を示すと説明されている
- 実装が非常に軽い
- その場で同期的に読める

実際の評価:

- Windows 10 / 従来の battery saver 互換経路としては妥当
- しかし Windows 11 24H2 の `Energy saver` を拾えないケースがあった

今回の観測:

- `省エネ機能 ON` にしても、メニュー側の判定が変化しなかった
- つまり `SystemStatusFlag` だけでは必要条件を満たせなかった

結論:

- fallback として残すのはよい
- ただし主判定には使わない

## 4. `GUID_ENERGY_SAVER_STATUS` + `PowerSettingRegisterNotification`

対象 API:

- `PowerSettingRegisterNotification`
- `PowerSettingUnregisterNotification`
- `GUID_ENERGY_SAVER_STATUS`

用途:

- `Energy saver` 状態の取得と変更通知

この方法が正解だった理由:

- `Energy saver` そのものの状態変化を表す GUID だから
- callback 登録直後に現在値が通知されるため、初期状態取得と監視が一体化できるから
- `STANDARD` / `HIGH_SAVINGS` の両方を扱えるから

Docs 上の意味:

- `ENERGY_SAVER_OFF`
- `ENERGY_SAVER_STANDARD`
- `ENERGY_SAVER_HIGH_SAVINGS`

実装上の扱い:

- `0` は OFF とみなす
- `0` 以外は `Energy saver active` とみなす

この扱いにしている理由:

- ドキュメント上は `OFF / STANDARD / HIGH_SAVINGS` の 3 状態だが、SDK header にまだ十分露出していない
- 現在の UI 要件は「ON か OFF か」が分かればよい
- 非 0 を一律 active とする方が将来の状態追加にも強い

## 実装上のポイント

## `GUID_ENERGY_SAVER_STATUS` は SDK header に見当たらない場合がある

今回確認した Windows SDK では `GUID_POWER_SAVING_STATUS` は定義されていたが、`GUID_ENERGY_SAVER_STATUS` は header 検索で見つからなかった。

そのため、現時点では GUID を手書きで定義する実装が必要になることがある。

使用した GUID:

```text
550E8400-E29B-41D4-A716-446655440000
```

この GUID は Microsoft Learn の `Power Setting GUIDs` に記載されている。

## callback 方式を使う

`PowerSettingRegisterNotification` には大きく 2 系統ある。

- window handle を渡して `WM_POWERBROADCAST` を受ける方法
- `DEVICE_NOTIFY_CALLBACK` を使って callback を受ける方法

このアプリでは callback 方式が適している。

理由:

- hidden window の message routing を増やさずに済む
- 初期値取得と更新通知を `src/power.rs` の責務に閉じ込めやすい
- メニュー表示側からは単に `is_energy_saver_active()` を呼ぶだけでよい

## 登録直後の即時 callback を利用する

`PowerSettingRegisterNotification` は登録成功直後に現在値を callback する。

これを使うと次の問題を避けられる。

- アプリ起動直後は状態不明で、最初の変化まで正しい判定ができない
- 起動直後の最初のメニュー表示だけ古い状態を使ってしまう

このアプリでは以下の流れにしている。

1. 起動時に `init_energy_saver_tracking()` を呼ぶ
2. callback で現在値を受ける
3. 短時間だけ待って初期キャッシュを得る
4. 以後は atomic に保存した値を参照する

## 監視値はキャッシュする

`is_energy_saver_active()` のたびに重い問い合わせをする必要はない。

今回の実装では、callback で受けた値を `AtomicU32` に保持している。

利点:

- メニュー表示時のコストが低い
- 毎回 registration / unregistration しなくてよい
- API 意味解釈の分岐が `power.rs` に閉じる

## fallback は残すが主経路にしない

callback 登録に失敗した場合や、初期 callback が取れなかった場合に備えて、`GetSystemPowerStatus().SystemStatusFlag` を fallback として残している。

ただし意味づけは明確にする。

- 主経路: `GUID_ENERGY_SAVER_STATUS`
- 補助経路: `SystemStatusFlag`

この優先順位を逆にしないこと。

## 現在の実装方針

このリポジトリでは現在、以下の責務分担にしている。

### `src/power.rs`

- overlay GUID API による current mode の取得と設定
- `GUID_ENERGY_SAVER_STATUS` callback の登録と解除
- 省エネ機能状態のキャッシュ
- `is_energy_saver_active()` の公開

### `src/main.rs`

- 起動時に `power::init_energy_saver_tracking()` を呼ぶ
- 終了時に `power::shutdown_energy_saver_tracking()` を呼ぶ

### `src/menu.rs`

- `power::is_energy_saver_active()` を見る
- active の場合は状態表示行を追加し、power mode メニューを `MF_GRAYED` にする

## 判定ロジックの設計ルール

将来この領域を触るときは、以下を守る。

1. `PowerMode` と `Energy saver` を同じ情報源から推測しない
2. `Best Power Efficiency` を見て `Energy saver` と決めつけない
3. `effective power mode` は UX 最適化や参考情報には使えても、トグル判定には使わない
4. `Energy saver` の ON/OFF 判定は専用 API から取る
5. ON/OFF だけ必要なら、詳細 enum を無理に広げず非 0 を active として扱う

## 既知の注意点

## 1. Microsoft Docs 上でも `GUID_ENERGY_SAVER_STATUS` は prerelease 扱いが含まれる

そのため、将来 SDK header への露出や定義名が変わる可能性はある。

対策:

- GUID 値そのものを設計メモに残しておく
- 「header に定義がないから使えない」と早合点しない

## 2. `GetSystemPowerStatus` が完全互換ではない

`Battery saver` と `Energy saver` を一括で同じ API から取れるとは考えないこと。

今回の調査では、そこを同一視すると誤実装になった。

## 3. callback 登録のライフサイクル管理が必要

登録したら終了時に解除する。

理由:

- リソースリーク回避
- 終了シーケンスを明確にするため

このアプリでは:

- `main()` で登録
- `WM_COMMAND` の Quit と `WM_DESTROY` の両方で解除

## 4. 値の完全列挙に依存しすぎない

`OFF / STANDARD / HIGH_SAVINGS` の 3 状態が現状の前提だが、実装上は「0 以外は active」としておくと安全側になる。

UI 要件が ON/OFF のみなら、この抽象度で十分である。

## 今回の学び

今回の不具合修正で得た本質的な学びは次の通り。

- Windows の電源関連 API は、名前が似ていても意味が異なる
- `power mode overlay` と `effective power mode` と `energy saver status` は別物
- 「似た値が返る」ことと「同じ状態を表している」ことは別
- Windows 11 の新しい `Energy saver` を扱うなら、専用の power setting GUID を使うのが最も確実

## 実装判断まとめ

このプロジェクトでの最終判断は次の通り。

- power mode の取得・設定: 既存の overlay GUID API を継続使用
- 省エネ機能状態の取得: `GUID_ENERGY_SAVER_STATUS` + `PowerSettingRegisterNotification`
- fallback: `GetSystemPowerStatus().SystemStatusFlag`
- メニュー無効化条件: `Energy saver active` のときのみ
- `Best Power Efficiency` 単独では無効化しない

この方針が、今回確認できた範囲では最も正確で副作用が少ない。

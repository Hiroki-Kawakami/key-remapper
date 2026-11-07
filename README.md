# key-remapper

macOS / Windows 用の最小構成キーリマッパー。

- macOS: 1対1の置き換えは `hidutil` の `UserKeyMapping`（デバイス別可、キーボード接続時に再適用）、単独押しは CGEventTap
- Windows: `WH_KEYBOARD_LL` フック + `SendInput`。デバイス別の指定はできない

```
key-remapper <config.toml>
```

## 設定ファイル

使えるキー名は `src/macos/keys.rs`（`HID_USAGES` が `remap` 用、`KEYCODES` が `tap` 用）と `src/windows/keys.rs` を参照。

### macOS

```toml
[[remap]]
from = "caps_lock"
to = "left_control"

[[device]]
vendor_id = 0x0000
product_id = 0x0000
remap = [
  { from = "left_command", to = "left_option" },
  { from = "left_option", to = "left_command" },
]

[[tap]]
key = "left_command"
alone = "japanese_eisuu"
timeout_ms = 1000
```

- `remap` は全キーボードに、`device` の `remap` は一致するキーボードにだけ適用する。両方に同じ `from` があれば `device` 側が優先
- `tap` は `hidutil` による置き換え後のキーに対して働く
- `timeout_ms` は省略時 1000。押してから離すまでがこれ以上なら単独押しとみなさない。他のキーやマウスボタンを押した場合も取り消す
- 終了時（SIGTERM / SIGINT / SIGHUP）に、自分が設定したデバイスの `UserKeyMapping` を空にする

### Windows

```toml
[[remap]]
from = "caps_lock"
to = "left_control"

[[tap]]
key = "left_alt"
alone = "japanese_eisuu"
```

`remap` の後に `tap` が適用される。`japanese_eisuu` / `japanese_kana` は `VK_IME_OFF` / `VK_IME_ON` を送る。

## macOS

```sh
nix build
./result/bin/key-remapper <config.toml>
```

アクセシビリティ権限が必要。権限がないとイベントタップの作成に失敗して終了する。

## Windows

```sh
cargo build --release
```

`key-remapper.exe <config.toml>` で起動するとタスクトレイに常駐する。設定エラーはダイアログで表示する。管理者権限のウィンドウにも効かせる場合は、タスクスケジューラで「ログオン時」「最上位の特権で実行する」として登録する。

# JONSBO DS339 Minimal

JONSBO DS339に、背景画像と時計を合成した映像を定期送信するための軽量な常駐アプリケーションです。

Windowsを最初の動作対象とし、Linux向けビルドにも対応できる構成を目指します。

## 現在の状態

DS339のHIDインターフェースを列挙し、接続できることを確認するCLIを実装しています。映像送信とGUIはまだ実装していません。

## 開発

Rustのstableツールチェーンを使用します。

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build
cargo run
```

`cargo run`はDS339のUSB ID `345F:9132`を検索し、HIDインターフェースを開けるか確認します。この段階ではデバイスへ制御コマンドや映像を送信しません。

## 実装予定

1. DS339の検出と接続
2. テスト画像の送信
3. 背景画像と時計の合成
4. 設定GUIとタスクトレイ常駐
5. USB切断・再接続とスリープ復帰への対応
6. Linuxでのビルドと実機通信の検証

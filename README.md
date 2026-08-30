# JONSBO DS339 Minimal

JONSBO DS339に、背景画像と時計を合成した映像を定期送信するための軽量な常駐アプリケーションです。

Windowsを最初の動作対象とし、Linux向けビルドにも対応できる構成を目指します。

## 現在の状態

DS339のHID制御インターフェースとUSB Bulk映像インターフェースを列挙し、接続できることを確認するCLIを実装しています。映像送信とGUIはまだ実装していません。

## 開発

Rustのstableツールチェーンを使用します。

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build
cargo run
cargo run -- --test-pattern
```

`cargo run`はDS339のUSB ID `345F:9132`を検索し、HIDインターフェースと映像用Bulk OUTエンドポイント`0x04`を開けるか確認します。HIDではパネル接続状態レジスターを読み取りますが、画面設定の変更や映像送信は行いません。

Windowsでは純正ドライバーが導入する`libusb0.dll`を動的に利用します。Linuxでは`rusb`（libusb 1.0）を利用する構成です。

`--test-pattern`を指定した場合だけ、DS339をネイティブ解像度に設定してRGBカラーバーを1フレーム送信します。通常起動では画面内容を変更しません。

期待されるネイティブフレームは次の画像です。実機の取り付け方向では90度回転して表示される場合があります。

![Expected RGB test pattern](docs/expected-test-pattern.svg)

## 実装予定

1. DS339の検出と接続
2. テスト画像の送信
3. 背景画像と時計の合成
4. 設定GUIとタスクトレイ常駐
5. USB切断・再接続とスリープ復帰への対応
6. Linuxでのビルドと実機通信の検証

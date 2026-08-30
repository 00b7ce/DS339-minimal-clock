# DS339 Minimal Clock

JONSBO DS339へ時計画像を定期送信する、非公式の軽量デスクトップアプリケーションです。背景画像・色、フォント、文字サイズ、文字色、配置をGUIで設定し、実際に送信するフレームをそのままプレビューできます。

> [!IMPORTANT]
> JONSBO公式ソフトウェアではありません。現在、実機での送信はWindows環境で確認済みです。Linux版はビルド可能な構成ですが、DS339実機での動作は未確認です。

## 主な機能

- 1秒ごとの時計表示（`HH:MM:SS`）
- 背景画像（PNG/JPEG）または背景色
- インストール済みフォントのドロップダウン選択
- フォントサイズ、文字色、中央配置または座標指定
- DS339へ送るフレームと同じ内容のプレビュー
- 複数プロファイルの保存・切り替えとバックアップ復旧
- システムトレイ常駐（Start / Apply、Stop、設定表示、終了）
- PCログイン時の自動起動、自動Start、トレイ格納
- USB接続確認やテストパターン送信を行うCLI

## Windowsで使う

1. [Releases](https://github.com/00b7ce/DS339-minimal-clock/releases)からWindows版ZIPをダウンロードして展開します。
2. DS339の純正ソフトウェア／ドライバーをセットアップします。
3. `jonsbo-ds339-gui.exe`を起動します。
4. 背景、フォント、サイズ、色、配置を調整し、`Start / Apply`を押します。

Windows版のUSB映像送信は`libusb0.dll`を動的に読み込みます。`libusb0.dll`が見つからない場合は、純正環境のインストール状況とDLLがWindowsの検索パスまたは実行ファイルと同じフォルダーに存在することを確認してください。DLL自体はこのリポジトリやReleaseへ同梱していません。

### プロファイル

`Save as`へ名前を入力して`Save`を押すと、現在の表示設定を保存します。選択中のプロファイルは次回起動時にも復元されます。

設定ファイルの保存先:

- Windows: `%APPDATA%\jonsbo-ds339-minimal\profiles.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/jonsbo-ds339-minimal/profiles.toml`

上書き保存時には`profiles.toml.bak`を作成し、メインファイルが壊れている場合はバックアップから復旧します。

### ログイン時の自動起動

`Start automatically at sign-in (hidden in the system tray)`を有効にすると、現在のユーザーでログインした際に次の動作を行います。

1. GUIを`--autostart`付きで起動
2. 設定ウィンドウを表示せずタスクトレイへ格納
3. 選択中のプロファイルでDS339への送信を開始

Windowsでは現在ユーザーの`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`、LinuxではXDG Autostartを使用します。チェックを外すと登録を削除します。

## CLI

`jonsbo-ds339-minimal`には診断用コマンドがあります。

```text
jonsbo-ds339-minimal                  # USB/HID接続確認（表示は変更しない）
jonsbo-ds339-minimal --test-pattern   # RGBテストパターンを送信
jonsbo-ds339-minimal --clock-once     # 時計を1フレーム送信
jonsbo-ds339-minimal --clock          # 1秒ごとに時計を送信（Ctrl+Cで停止）
jonsbo-ds339-minimal --gui            # GUIを起動（コンソールあり）
```

`--test-pattern`で期待される論理表示は次の画像です。横置きでは実機側の向きに合わせて回転して見える場合があります。

![Expected RGB test pattern](docs/expected-test-pattern.png)

## Linux

Release ActionはUbuntu上でLinux x86_64版をビルドします。Ubuntu/Debianでローカルビルドする場合の主なネイティブ依存関係は次のとおりです。

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libudev-dev libusb-1.0-0-dev \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

その後、Rust stableでビルドします。

```bash
cargo build --release --bins
```

LinuxではUSBデバイスへのアクセス権限（udevルールまたは適切なグループ設定）が別途必要になる場合があります。現時点ではLinux実機送信を未検証のため、Release版も実験的扱いです。

## 開発と検証

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --bins
```

対象としているDS339のUSB情報はVID `345F`、PID `9132`、映像用インターフェース`3`、Bulk OUTエンドポイント`0x04`です。論理プレビューは横向き`960x376`で生成し、実機送信用のネイティブ向きへ変換します。

## 既知の制限

- Windows以外での実機動作は未確認です。
- 複数台のDS339を個別選択するUIはありません。
- スリープ復帰後の自動再接続は未実装です。エラー時はトレイまたはGUIから再度`Start / Apply`してください。
- 署名付きインストーラーや自動更新機能はありません。

## License

[MIT License](LICENSE)

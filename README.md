# NeLST (Network Load and Security Test)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

ネットワークの負荷テストとセキュリティテストを行うCLIツール。

## 特徴

- 🚀 **負荷テスト**: トラフィック負荷テスト、コネクション負荷テスト、HTTP負荷テスト
- 🌐 **HTTP対応**: GET/POST/PUT/DELETE、カスタムヘッダー、HTTP/2サポート
- 🔍 **セキュリティスキャン**: ポートスキャン（TCP Connect, SYN, FIN, Xmas, NULL, UDP）
- 🖥️ **テストサーバ**: エコーサーバ、シンクサーバ、フラッドサーバ、HTTPサーバ
- 📊 **詳細な統計**: レイテンシ（P50/P95/P99）、スループット、成功率
- 📁 **複数の出力形式**: テキスト、JSON、ファイル出力

## インストール

### ソースからビルド

```bash
git clone https://github.com/your-username/NeLST.git
cd NeLST
cargo build --release
```

ビルドされたバイナリは `target/release/nelst` に配置されます。

### 必要要件

- Rust 1.85以上（Edition 2024）
- Linux / macOS / Windows

## 使用方法

### 基本的なコマンド構造

```
nelst <COMMAND> [OPTIONS]

COMMANDS:
    load        負荷テスト（トラフィック/コネクション）
    scan        セキュリティスキャン（ポートスキャン）
    server      テスト用サーバを起動
    help        ヘルプ表示

GLOBAL OPTIONS:
    -v, --verbose        詳細ログを出力
    -q, --quiet          出力を最小限に抑える
        --json           JSON形式で出力
        --config <FILE>  設定ファイルを指定
```

### 負荷テスト

#### トラフィック負荷テスト

ターゲットへ指定サイズのパケットを送信し続けます。

```bash
# TCPでエコーサーバへ60秒間負荷テスト
nelst load traffic -t 192.168.1.100:8080 -d 60 -s 4096 -m echo

# 10並列で送信のみ
nelst load traffic -t 192.168.1.100:8080 -c 10 -m send
```

#### コネクション負荷テスト

大量のTCPコネクションを確立し、サーバのコネクション処理能力をテストします。

```bash
# 1000コネクションを100並列で確立
nelst load connection -t 192.168.1.100:8080 -n 1000 -c 100
```

#### HTTP負荷テスト

HTTPサーバへ継続的にリクエストを送信し負荷テストを行います。

```bash
# 基本的なGETリクエスト負荷テスト（60秒、10並列）
nelst load http -u http://192.168.1.100:8080/api -d 60 -c 10

# POSTリクエスト with カスタムヘッダー
nelst load http -u http://192.168.1.100:8080/api \
  -X POST \
  -H "Content-Type: application/json" \
  -b '{"key":"value"}'

# レート制限付き（100 req/s）、結果をファイルに保存
nelst load http -u http://192.168.1.100:8080 -r 100 -o result.json

# SSL証明書検証スキップ、リダイレクト追従
nelst load http -u https://example.com --insecure --follow-redirects

# HTTP/2を優先使用
nelst load http -u https://example.com --http2
```

### セキュリティスキャン

#### ポートスキャン

```bash
# 標準的なTCP Connectスキャン
nelst scan port -t 192.168.1.100 --ports 1-1024

# 特定ポートのみスキャン
nelst scan port -t 192.168.1.100 --ports 22,80,443,8080

# 並列度を上げてスキャン
nelst scan port -t 192.168.1.100 --ports 1-65535 -c 500
```

### テストサーバ

負荷テストのターゲットとして使用できるサーバを起動します。

```bash
# エコーサーバ（受信データをそのまま返す）
nelst server echo -b 0.0.0.0:8080

# UDPエコーサーバ
nelst server echo -b 0.0.0.0:8080 -p udp

# シンクサーバ（受信のみ、応答なし）
nelst server sink -b 0.0.0.0:8080

# フラッドサーバ（接続元へデータを送信し続ける）
nelst server flood -b 0.0.0.0:8080 -s 4096

# HTTPテストサーバ
nelst server http -b 0.0.0.0:8080

# HTTPサーバ with 遅延シミュレーション（50ms）とエラー率（10%）
nelst server http -b 0.0.0.0:8080 --delay 50 --error-rate 0.1
```

## 設定ファイル

`~/.nelst/config.toml` または `./nelst.toml` に設定を記述できます。

```toml
[defaults]
timeout = 5000
verbose = false

[load]
protocol = "tcp"
concurrency = 10
duration = 60
size = 1024

[scan]
method = "tcp"
ports = "1-1024"
concurrency = 100
timeout = 1000

[server]
bind = "0.0.0.0:8080"
protocol = "tcp"
```

## 出力例

### テキスト出力

```
NeLST - Network Load Test
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Target:         192.168.1.100:8080
  Protocol:       tcp
  Mode:           echo
  Duration:       60s
  Concurrency:    10

━━━━━━━━━━━━━━━━━━━ RESULTS ━━━━━━━━━━━━━━━━━━━

  Total Requests:          12345
  Successful:              12300 (99.64%)
  Failed:                     45 (0.36%)

  Throughput:             205.75 req/s
  Data Transferred:       12.05 MB

  Latency:
    Min:        0.45 ms
    Max:       25.32 ms
    Avg:        4.87 ms
    P50:        4.12 ms
    P95:       12.56 ms
    P99:       18.23 ms
```

### JSON出力

`--json` オプションを付けると、結果をJSON形式で出力します。

```bash
nelst load traffic -t 192.168.1.100:8080 -d 10 --json
```

## 開発

### ビルド

```bash
cargo build
```

### テスト

```bash
cargo test
```

### Lint

```bash
cargo clippy
```

### フォーマット

```bash
cargo fmt
```

## プロジェクト構造

```
src/
├── main.rs           # エントリーポイント
├── cli/              # CLIパーサー
│   ├── mod.rs
│   ├── load.rs       # 負荷テストコマンド
│   ├── scan.rs       # スキャンコマンド
│   └── server.rs     # サーバコマンド
├── common/           # 共通モジュール
│   ├── config.rs     # 設定管理
│   ├── error.rs      # エラーハンドリング
│   ├── output.rs     # 出力ユーティリティ
│   └── stats.rs      # 統計収集
├── load/             # 負荷テスト実装
│   ├── traffic.rs    # トラフィック負荷テスト
│   ├── connection.rs # コネクション負荷テスト
│   └── http.rs       # HTTP負荷テスト
├── scan/             # スキャン実装
│   └── tcp_connect.rs
└── server/           # サーバ実装
    ├── echo.rs       # エコーサーバ
    ├── sink.rs       # シンクサーバ
    ├── flood.rs      # フラッドサーバ
    └── http.rs       # HTTPサーバ
```

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) ファイルを参照してください。

## 貢献

Issue や Pull Request を歓迎します。

## ロードマップ

- [x] 基盤整備（v0.0.x）
- [x] MVP - 基本機能（v0.1.0）
  - トラフィック/コネクション負荷テスト
  - TCP Connectポートスキャン
  - Echo/Sink/Floodサーバ
- [x] HTTP負荷テスト、UDP対応（v0.2.0）
  - HTTP負荷テスト（GET/POST/PUT/DELETE、HTTP/2）
  - HTTPテストサーバ（遅延・エラー率シミュレーション）
  - 結果ファイル出力
  - レート制限
- [ ] SYN/FIN/Xmasスキャン、SSL検査（v0.3.0）
- [ ] 診断機能（ping/traceroute/DNS）（v0.4.0）
- [ ] レポート機能、プロファイル管理（v0.5.0)

詳細は [doc/DESIGN.md](doc/DESIGN.md) および [doc/PLAN.md](doc/PLAN.md) を参照してください。


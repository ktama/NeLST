# NeLST (Network Load and Security Test)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/your-username/NeLST/workflows/CI/badge.svg)](https://github.com/your-username/NeLST/actions)

ネットワークの負荷テストとセキュリティテストを行うCLIツール。

## 特徴

- 🚀 **負荷テスト**: トラフィック負荷テスト、コネクション負荷テスト、HTTP負荷テスト
- 🌐 **HTTP対応**: GET/POST/PUT/DELETE、カスタムヘッダー、HTTP/2サポート
- 🔍 **セキュリティスキャン**: ポートスキャン（TCP Connect, SYN, FIN, Xmas, NULL, UDP）
- 🔐 **SSL/TLS検査**: 証明書情報取得、有効期限チェック、暗号スイート検査
- 🏷️ **サービス検出**: バナー取得、サービス識別、バージョン検出
- 📡 **ネットワーク診断**: Ping（ICMP/TCP）、Traceroute、DNS解決、MTU探索
- 📈 **帯域幅測定**: 帯域幅測定、レイテンシ測定、ヒストグラム表示
- 🖥️ **テストサーバ**: エコーサーバ、シンクサーバ、フラッドサーバ、HTTPサーバ
- 📊 **詳細な統計**: レイテンシ（P50/P95/P99）、スループット、成功率
- 📁 **複数の出力形式**: テキスト、JSON、CSV、HTML、Markdown
- 🗒️ **プロファイル管理**: 設定の保存・再利用、エクスポート/インポート

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
    load        負荷テスト（トラフィック/コネクション/HTTP）
    scan        セキュリティスキャン（ポートスキャン）
    diag        ネットワーク診断（ping/traceroute/DNS/MTU）
    bench       帯域幅・レイテンシ測定
    server      テスト用サーバを起動
    help        ヘルプ表示

GLOBAL OPTIONS:
    -v, --verbose           詳細ログを出力
    -q, --quiet             出力を最小限に抑える
        --json              JSON形式で出力
        --config <FILE>     設定ファイルを指定
        --profile <NAME>    プロファイルを使用
        --save-profile <NAME>  現在の設定をプロファイルとして保存
        --format <FORMAT>   出力形式 [json|csv|html|markdown|text]
        --report <FILE>     結果をファイルに保存
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

# SYNスキャン（root権限が必要）
sudo nelst scan port -t 192.168.1.100 -m syn --ports 1-1024

# FIN/Xmas/NULLスキャン（ステルス性が高い）
sudo nelst scan port -t 192.168.1.100 -m fin
sudo nelst scan port -t 192.168.1.100 -m xmas
sudo nelst scan port -t 192.168.1.100 -m null

# UDPスキャン
nelst scan port -t 192.168.1.100 -m udp --ports 53,123,161,500
```

#### サービス検出・バナー取得

```bash
# サービス検出を有効にしてスキャン
nelst scan port -t 192.168.1.100 --ports 1-1000 --service-detection

# バナー取得を有効にしてスキャン
nelst scan port -t 192.168.1.100 --ports 22,80,443 --grab-banner
```

#### SSL/TLS検査

```bash
# オープンしているSSLポートのTLS情報と証明書を検査
nelst scan port -t 192.168.1.100 --ports 443,8443 --ssl-check

# ホスト名を指定してSSL証明書検証
nelst scan port -t 192.168.1.100 --ports 443 --ssl-check --hostname example.com
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

### ネットワーク診断

#### Ping

```bash
# 通常のICMP ping
sudo nelst diag ping -t google.com -c 5

# TCP ping（ファイアウォール越しなど）
nelst diag ping -t google.com --tcp --port 443

# パケットサイズ指定
sudo nelst diag ping -t 192.168.1.1 -c 10 -s 1024
```

#### Traceroute

```bash
# 経路追跡（デフォルトはUDP）
nelst diag trace -t google.com

# TCPモードで特定ポートへ
nelst diag trace -t google.com --tcp --port 443

# ICMPモードで最大15ホップ
sudo nelst diag trace -t google.com --icmp --max-hops 15
```

#### DNS解決

```bash
# Aレコード検索
nelst diag dns -t google.com

# MXレコード検索
nelst diag dns -t google.com --record-type mx

# すべてのレコードタイプを検索
nelst diag dns -t google.com --record-type all

# 特定のDNSサーバを使用
nelst diag dns -t google.com -s 8.8.8.8

# TCPで問い合わせ
nelst diag dns -t google.com --tcp
```

#### MTU探索

```bash
# Path MTU探索
sudo nelst diag mtu -t google.com

# 探索範囲を指定
sudo nelst diag mtu -t 192.168.1.1 --min-mtu 576 --max-mtu 9000
```

### 帯域幅・レイテンシ測定

#### 帯域幅測定

```bash
# サーバモードで起動
nelst bench bandwidth --server -b 0.0.0.0:5201

# クライアントとして測定（10秒間）
nelst bench bandwidth -t 192.168.1.100:5201 -d 10

# アップロード/ダウンロード両方を測定
nelst bench bandwidth -t 192.168.1.100:5201 --direction both

# 並列ストリーム数を指定
nelst bench bandwidth -t 192.168.1.100:5201 -p 4
```

#### レイテンシ測定

```bash
# サーバモードで起動
nelst bench latency --server -b 0.0.0.0:5201

# レイテンシ測定（100回）
nelst bench latency -t 192.168.1.100:5201 -c 100

# 継続時間指定で測定
nelst bench latency -t 192.168.1.100:5201 -d 60

# ヒストグラム表示
nelst bench latency -t 192.168.1.100:5201 -d 60 --histogram

# 詳細統計を出力
nelst bench latency -t 192.168.1.100:5201 -c 1000 --json
```

### プロファイル管理

よく使う設定をプロファイルとして保存・管理できます。

```bash
# 実行時にプロファイルとして保存
nelst load traffic -t 192.168.1.100:8080 -c 50 -d 60 --save-profile prod-load-test

# 保存したプロファイルを使用して実行
nelst load traffic --profile prod-load-test

# プロファイル一覧を表示
nelst profile list

# プロファイル詳細を表示
nelst profile show my-scan

# プロファイルを削除
nelst profile delete old-profile

# プロファイルをファイルにエクスポート
nelst profile export my-scan -o my-scan.toml

# ファイルからプロファイルをインポート
nelst profile import shared-scan.toml --name imported-scan
```

### レポート出力

テスト結果を複数のフォーマットで出力できます。

```bash
# HTML形式でレポートを保存
nelst load http -u http://192.168.1.100:8080 --format html --report result.html

# CSV形式で保存
nelst scan port -t 192.168.1.100 --ports 1-1024 --format csv --report scan-result.csv

# Markdown形式で保存
nelst bench latency -t 192.168.1.100:5201 --format markdown --report latency.md

# JSON形式（デフォルト）で保存
nelst diag dns -t google.com --format json --report dns-result.json
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
│   ├── diag.rs       # 診断コマンド
│   ├── bench.rs      # ベンチマークコマンド
│   ├── server.rs     # サーバコマンド
│   └── profile.rs    # プロファイルコマンド
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
│   ├── tcp_connect.rs # TCP Connectスキャン
│   ├── syn.rs        # SYN/FIN/Xmas/NULLスキャン
│   ├── udp.rs        # UDPスキャン
│   ├── raw_socket.rs # Raw Socket基盤
│   ├── service.rs    # サービス検出
│   └── ssl.rs        # SSL/TLS検査
├── diag/             # ネットワーク診断
│   ├── ping.rs       # Ping
│   ├── trace.rs      # Traceroute
│   ├── dns.rs        # DNS解決
│   └── mtu.rs        # MTU探索
├── bench/            # ベンチマーク
│   ├── bandwidth.rs  # 帯域幅測定
│   └── latency.rs    # レイテンシ測定
├── server/           # サーバ実装
│   ├── echo.rs       # エコーサーバ
│   ├── sink.rs       # シンクサーバ
│   ├── flood.rs      # フラッドサーバ
│   └── http.rs       # HTTPサーバ
├── report/           # レポート機能
│   └── formatter.rs  # 出力フォーマッタ
└── profile/          # プロファイル管理
    └── manager.rs    # プロファイルマネージャ
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
- [x] セキュリティ機能強化（v0.3.0）
  - SYN/FIN/Xmas/NULLスキャン（Raw Socket）
  - UDPスキャン
  - サービス検出・バナー取得
  - SSL/TLS検査（証明書・暗号スイート）
- [x] 診断機能（v0.4.0）
  - Ping（ICMP/TCP）
  - Traceroute（UDP/TCP/ICMP）
  - DNS解決（全レコードタイプ）
  - MTU探索
  - 帯域幅・レイテンシ測定
- [x] 運用機能（v0.5.0）
  - プロファイル管理
  - レポート機能（HTML/CSV/Markdown）
  - 設定ファイル対応
  - CI/CD（GitHub Actions）

詳細は [doc/DESIGN.md](doc/DESIGN.md) および [doc/PLAN.md](doc/PLAN.md) を参照してください。


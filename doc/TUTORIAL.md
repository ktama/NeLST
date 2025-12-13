# NeLST チュートリアル & ハンズオン

このドキュメントでは、NeLST（Network Load and Security Test）の全機能を段階的に学習できるハンズオン形式のチュートリアルを提供します。

---

## 📚 目次

1. [はじめに](#1-はじめに)
2. [インストールと動作確認](#2-インストールと動作確認)
3. [レベル1: 基礎 - テストサーバの起動](#3-レベル1-基礎---テストサーバの起動)
4. [レベル2: 負荷テスト入門](#4-レベル2-負荷テスト入門)
5. [レベル3: HTTP負荷テスト](#5-レベル3-http負荷テスト)
6. [レベル4: ポートスキャン](#6-レベル4-ポートスキャン)
7. [レベル5: 高度なスキャン技術](#7-レベル5-高度なスキャン技術)
8. [レベル6: ネットワーク診断](#8-レベル6-ネットワーク診断)
9. [レベル7: 帯域幅・レイテンシ測定](#9-レベル7-帯域幅レイテンシ測定)
10. [レベル8: プロファイル管理](#10-レベル8-プロファイル管理)
11. [レベル9: レポート出力](#11-レベル9-レポート出力)
12. [機能一覧クイックリファレンス](#12-機能一覧クイックリファレンス)

---

## 1. はじめに

### NeLSTとは？

NeLSTは、ネットワークの負荷テストとセキュリティテストを行うためのオールインワンCLIツールです。

### 主な機能カテゴリ

| カテゴリ | コマンド | 概要 |
|---------|---------|------|
| 🚀 負荷テスト | `nelst load` | トラフィック、コネクション、HTTP負荷テスト |
| 🔍 スキャン | `nelst scan` | ポートスキャン、SSL/TLS検査、サービス検出 |
| 📡 診断 | `nelst diag` | Ping、Traceroute、DNS、MTU探索 |
| 📈 測定 | `nelst bench` | 帯域幅、レイテンシ測定 |
| 🖥️ サーバ | `nelst server` | テスト用サーバ起動 |
| 📁 プロファイル | `nelst profile` | 設定の保存・管理 |

---

## 2. インストールと動作確認

### 2.1 ビルド

```bash
# リポジトリをクローン
git clone https://github.com/your-username/NeLST.git
cd NeLST

# リリースビルド
cargo build --release

# バイナリを確認
./target/release/nelst --version
```

### 2.2 ヘルプの確認

```bash
# 全体のヘルプ
nelst --help

# サブコマンドのヘルプ
nelst load --help
nelst load traffic --help
```

### 2.3 グローバルオプション

すべてのコマンドで使用可能なオプション：

| オプション | 説明 |
|-----------|------|
| `-v, --verbose` | 詳細ログを出力 |
| `-q, --quiet` | 出力を最小限に |
| `--json` | JSON形式で出力 |
| `--config <FILE>` | 設定ファイルを指定 |
| `--profile <NAME>` | プロファイルを使用 |
| `--save-profile <NAME>` | 設定をプロファイルとして保存 |
| `--format <FORMAT>` | 出力形式（json/csv/html/markdown/text） |
| `--report <FILE>` | 結果をファイルに保存 |

---

## 3. レベル1: 基礎 - テストサーバの起動

負荷テストを行うには、まずターゲットとなるサーバが必要です。NeLSTには4種類のテストサーバが内蔵されています。

### 3.1 エコーサーバ（Echo Server）

受信したデータをそのまま返すサーバ。負荷テストの基本となるサーバです。

```bash
# ターミナル1: エコーサーバを起動
nelst server echo -b 0.0.0.0:8080

# 出力例：
# 🖥️  Echo Server
# ━━━━━━━━━━━━━━━━━━━━━━━━
#   Bind:      0.0.0.0:8080
#   Protocol:  tcp
#
# Server is running. Press Ctrl+C to stop.
```

**✅ 確認ポイント**
- `-b`: バインドアドレス（リッスンするアドレスとポート）
- `-p udp`: UDPモードで起動することも可能

```bash
# UDPエコーサーバ
nelst server echo -b 0.0.0.0:8080 -p udp
```

### 3.2 シンクサーバ（Sink Server）

受信のみ行い、応答を返さないサーバ。送信専用の負荷テストに使用。

```bash
nelst server sink -b 0.0.0.0:8081
```

### 3.3 フラッドサーバ（Flood Server）

接続してきたクライアントに対してデータを送り続けるサーバ。受信テストに使用。

```bash
# 4KBのデータを送り続ける
nelst server flood -b 0.0.0.0:8082 -s 4096
```

### 3.4 HTTPサーバ

HTTP負荷テスト用のサーバ。遅延やエラー率をシミュレート可能。

```bash
# 基本的なHTTPサーバ
nelst server http -b 0.0.0.0:8080

# 50ms遅延 + 10%エラー率をシミュレート
nelst server http -b 0.0.0.0:8080 --delay 50 --error-rate 0.1

# カスタムレスポンス
nelst server http -b 0.0.0.0:8080 --body "Hello, World!" --status 200
```

### 🎯 ハンズオン課題1

1. エコーサーバを8080ポートで起動してください
2. 別のターミナルから `nc localhost 8080` で接続し、文字を入力して返ってくることを確認してください
3. Ctrl+Cでサーバを停止してください

---

## 4. レベル2: 負荷テスト入門

### 4.1 トラフィック負荷テスト

データの送受信を繰り返し、スループットを測定します。

```bash
# ターミナル1: エコーサーバ起動
nelst server echo -b 0.0.0.0:8080

# ターミナル2: 負荷テスト実行
nelst load traffic -t 127.0.0.1:8080 -d 10
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target` | `-t` | ターゲットアドレス | 必須 |
| `--duration` | `-d` | テスト時間（秒） | 60 |
| `--concurrency` | `-c` | 同時接続数 | 1 |
| `--size` | `-s` | パケットサイズ（バイト） | 1024 |
| `--mode` | `-m` | 動作モード（echo/send/recv） | echo |
| `--rate` | `-r` | 毎秒リクエスト数 | 無制限 |
| `--protocol` | `-p` | プロトコル（tcp/udp） | tcp |

**動作モード：**
- `echo`: サーバにデータを送信し、返答を受信（往復）
- `send`: 送信のみ
- `recv`: 受信のみ

```bash
# 10並列で4KBパケットを送信（エコーモード）
nelst load traffic -t 127.0.0.1:8080 -d 30 -c 10 -s 4096 -m echo

# 毎秒100リクエストに制限
nelst load traffic -t 127.0.0.1:8080 -d 30 -r 100
```

### 4.2 コネクション負荷テスト

大量のTCPコネクションを確立し、サーバのコネクション処理能力をテストします。

```bash
# 1000コネクションを100並列で確立
nelst load connection -t 127.0.0.1:8080 -n 1000 -c 100

# コネクションを維持したまま保持
nelst load connection -t 127.0.0.1:8080 -n 500 --keep-alive
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--count` | `-n` | コネクション総数 | 1000 |
| `--concurrency` | `-c` | 同時接続数 | 100 |
| `--keep-alive` | - | コネクションを維持 | false |
| `--timeout` | - | タイムアウト（ms） | 5000 |

### 🎯 ハンズオン課題2

1. エコーサーバを起動
2. 5並列、30秒間、2048バイトのトラフィック負荷テストを実行
3. 出力される統計情報（スループット、レイテンシ）を確認

```bash
# 解答例
nelst server echo -b 0.0.0.0:8080  # ターミナル1
nelst load traffic -t 127.0.0.1:8080 -d 30 -c 5 -s 2048  # ターミナル2
```

---

## 5. レベル3: HTTP負荷テスト

REST API やWebサーバへの負荷テストを行います。

### 5.1 基本的なHTTP負荷テスト

```bash
# ターミナル1: HTTPサーバ起動
nelst server http -b 0.0.0.0:8080

# ターミナル2: GET負荷テスト
nelst load http -u http://127.0.0.1:8080/ -d 30 -c 10
```

### 5.2 HTTPメソッドとヘッダー

```bash
# POSTリクエスト with JSONボディ
nelst load http -u http://127.0.0.1:8080/api \
  -X POST \
  -H "Content-Type: application/json" \
  -b '{"name": "test", "value": 123}'

# 認証ヘッダー付き
nelst load http -u http://127.0.0.1:8080/api \
  -H "Authorization: Bearer your-token-here"
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--url` | `-u` | ターゲットURL | 必須 |
| `--method` | `-X` | HTTPメソッド | GET |
| `--header` | `-H` | カスタムヘッダー | - |
| `--body` | `-b` | リクエストボディ | - |
| `--duration` | `-d` | テスト時間（秒） | 60 |
| `--concurrency` | `-c` | 同時接続数 | 1 |
| `--rate` | `-r` | 毎秒リクエスト数 | 無制限 |
| `--follow-redirects` | - | リダイレクト追跡 | false |
| `--insecure` | - | SSL検証スキップ | false |
| `--http2` | - | HTTP/2を優先 | false |
| `--timeout` | - | タイムアウト（ms） | 30000 |

### 5.3 高度なHTTPテスト

```bash
# HTTP/2で負荷テスト
nelst load http -u https://example.com --http2 -c 20 -d 60

# SSL証明書検証スキップ（自己署名証明書対応）
nelst load http -u https://localhost:8443 --insecure

# ファイルからボディを読み込み
nelst load http -u http://127.0.0.1:8080/api -X POST -b @request.json

# 結果をファイルに保存
nelst load http -u http://127.0.0.1:8080 -o result.json
```

### 🎯 ハンズオン課題3

1. 100ms遅延のHTTPサーバを起動
2. 5並列でGET負荷テストを20秒間実行
3. レイテンシが100ms以上になることを確認

```bash
# 解答例
nelst server http -b 0.0.0.0:8080 --delay 100  # ターミナル1
nelst load http -u http://127.0.0.1:8080/ -d 20 -c 5  # ターミナル2
```

---

## 6. レベル4: ポートスキャン

### 6.1 TCP Connectスキャン（基本）

最も基本的なスキャン方法。root権限不要。

```bash
# よく使うポートをスキャン
nelst scan port -t 127.0.0.1 --ports 22,80,443,8080

# ポート範囲でスキャン
nelst scan port -t 127.0.0.1 --ports 1-1024

# 全ポートスキャン（高並列）
nelst scan port -t 127.0.0.1 --ports 1-65535 -c 500
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target` | `-t` | ターゲットIP | 必須 |
| `--ports` | - | ポート範囲 | 1-1024 |
| `--method` | `-m` | スキャン方法 | tcp |
| `--concurrency` | `-c` | 並列数 | 100 |
| `--timeout` | - | タイムアウト（ms） | 1000 |
| `--top-ports` | - | 上位Nポートのみ | - |

**スキャン方法（-m オプション）：**

| メソッド | 説明 | 権限 |
|---------|------|------|
| `tcp` | TCP Connect | 不要 |
| `syn` | SYNスキャン | root必要 |
| `fin` | FINスキャン | root必要 |
| `xmas` | Xmasスキャン | root必要 |
| `null` | NULLスキャン | root必要 |
| `udp` | UDPスキャン | 不要 |

### 6.2 サービス検出とバナー取得

```bash
# サービス検出を有効化
nelst scan port -t 127.0.0.1 --ports 22,80,443 --service-detection

# バナー取得
nelst scan port -t 127.0.0.1 --ports 22,80 --grab-banner
```

### 6.3 SSL/TLS検査

```bash
# SSL証明書情報を取得
nelst scan port -t 127.0.0.1 --ports 443 --ssl-check

# ホスト名を指定してSNI対応
nelst scan port -t 93.184.216.34 --ports 443 --ssl-check --hostname example.com
```

### 🎯 ハンズオン課題4

1. エコーサーバを8080ポートで起動
2. 8000-8100の範囲でポートスキャンを実行
3. 8080ポートがopenとして検出されることを確認

```bash
# 解答例
nelst server echo -b 0.0.0.0:8080  # ターミナル1
nelst scan port -t 127.0.0.1 --ports 8000-8100  # ターミナル2
```

---

## 7. レベル5: 高度なスキャン技術

### 7.1 SYNスキャン（ステルススキャン）

フルTCPコネクションを確立せず、SYNパケットのみを送信。ログに残りにくい。

```bash
# root権限が必要
sudo nelst scan port -t 192.168.1.100 -m syn --ports 1-1024
```

### 7.2 FIN/Xmas/NULLスキャン

ファイアウォール回避に使用されるステルス技術。

```bash
# FINスキャン（FINフラグのみ）
sudo nelst scan port -t 192.168.1.100 -m fin --ports 1-1024

# Xmasスキャン（FIN+PSH+URGフラグ）
sudo nelst scan port -t 192.168.1.100 -m xmas --ports 1-1024

# NULLスキャン（フラグなし）
sudo nelst scan port -t 192.168.1.100 -m null --ports 1-1024
```

### 7.3 UDPスキャン

```bash
# DNS, NTP, SNMPなどのUDPサービスを検出
nelst scan port -t 192.168.1.100 -m udp --ports 53,123,161,500
```

### 7.4 総合スキャン例

```bash
# セキュリティ監査向け：フルスキャン + サービス検出 + SSL検査
nelst scan port -t 192.168.1.100 \
  --ports 1-65535 \
  -c 500 \
  --service-detection \
  --ssl-check \
  -o full-scan-result.json
```

---

## 8. レベル6: ネットワーク診断

### 8.1 Ping

```bash
# ICMP ping（root権限が必要な場合あり）
sudo nelst diag ping -t google.com -c 5

# TCP ping（ファイアウォール越しでも使用可能）
nelst diag ping -t google.com --tcp --port 443

# パケットサイズ指定
sudo nelst diag ping -t 192.168.1.1 -c 10 -s 1024
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target` | `-t` | ターゲットホスト | 必須 |
| `--count` | `-c` | 送信回数 | 4 |
| `--interval` | `-i` | 送信間隔（ms） | 1000 |
| `--timeout` | - | タイムアウト（ms） | 5000 |
| `--tcp` | - | TCP pingを使用 | false |
| `--port` | - | TCPポート | 80 |
| `--size` | `-s` | パケットサイズ | 64 |

### 8.2 Traceroute

経路上のホップを追跡します。

```bash
# UDP モード（デフォルト）
nelst diag trace -t google.com

# TCP モード
nelst diag trace -t google.com --tcp --port 443

# ICMP モード（root権限必要）
sudo nelst diag trace -t google.com --icmp

# 最大ホップ数を指定
nelst diag trace -t google.com --max-hops 20
```

### 8.3 DNS解決

```bash
# Aレコード（IPv4）
nelst diag dns -t google.com

# 様々なレコードタイプ
nelst diag dns -t google.com --record-type aaaa   # IPv6
nelst diag dns -t google.com --record-type mx     # メールサーバ
nelst diag dns -t google.com --record-type txt    # TXTレコード
nelst diag dns -t google.com --record-type ns     # ネームサーバ
nelst diag dns -t google.com --record-type all    # すべて

# 特定のDNSサーバを使用
nelst diag dns -t google.com -s 8.8.8.8

# TCP経由で問い合わせ
nelst diag dns -t google.com --tcp
```

### 8.4 MTU探索

Path MTU（最大転送単位）を探索します。

```bash
# MTU探索
sudo nelst diag mtu -t google.com

# 探索範囲を指定
sudo nelst diag mtu -t 192.168.1.1 --min-mtu 576 --max-mtu 9000
```

### 🎯 ハンズオン課題5

1. `google.com` に対してDNS解決を実行（すべてのレコードタイプ）
2. 取得されたIPアドレスの1つに対してtracerouteを実行
3. 何ホップで到達するか確認

```bash
# 解答例
nelst diag dns -t google.com --record-type all
nelst diag trace -t google.com --max-hops 30
```

---

## 9. レベル7: 帯域幅・レイテンシ測定

### 9.1 帯域幅測定

iperfライクな帯域幅測定機能。

```bash
# サーバモード（測定を受け付ける側）
nelst bench bandwidth --server -b 0.0.0.0:5201

# クライアントモード（測定を実行する側）
nelst bench bandwidth -t 127.0.0.1:5201 -d 10

# アップロードのみ
nelst bench bandwidth -t 127.0.0.1:5201 --direction up

# ダウンロードのみ
nelst bench bandwidth -t 127.0.0.1:5201 --direction down

# 並列ストリーム数を増やす
nelst bench bandwidth -t 127.0.0.1:5201 -p 4
```

**主要オプション：**

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target` | `-t` | ターゲットサーバ | - |
| `--server` | - | サーバモード | false |
| `--bind` | `-b` | バインドアドレス | 0.0.0.0:5201 |
| `--duration` | `-d` | 測定時間（秒） | 10 |
| `--direction` | - | 方向（up/down/both） | both |
| `--parallel` | `-p` | 並列ストリーム数 | 1 |
| `--block-size` | - | ブロックサイズ | 131072 |

### 9.2 レイテンシ測定

詳細なレイテンシ統計を取得します。

```bash
# 基本的なレイテンシ測定
nelst bench latency -t 127.0.0.1:5201 -d 60

# ヒストグラム表示
nelst bench latency -t 127.0.0.1:5201 -d 60 --histogram

# 測定間隔を指定（50ms間隔）
nelst bench latency -t 127.0.0.1:5201 -i 50
```

**出力される統計：**
- Min / Max / Avg レイテンシ
- P50, P95, P99 パーセンタイル
- 標準偏差
- ヒストグラム（オプション）

### 🎯 ハンズオン課題6

1. ターミナル1で帯域幅測定サーバを起動
2. ターミナル2から10秒間の帯域幅測定を実行
3. アップロードとダウンロードの速度を確認

```bash
# 解答例
nelst bench bandwidth --server -b 0.0.0.0:5201  # ターミナル1
nelst bench bandwidth -t 127.0.0.1:5201 -d 10 --direction both  # ターミナル2
```

---

## 10. レベル8: プロファイル管理

よく使う設定をプロファイルとして保存し、再利用できます。

### 10.1 プロファイルの保存

コマンド実行時に `--save-profile` オプションを追加すると、そのオプションがプロファイルとして保存されます。

```bash
# 負荷テスト設定を保存
nelst load traffic -t 192.168.1.100:8080 -c 50 -d 60 -s 4096 \
  --save-profile production-load-test

# スキャン設定を保存
nelst scan port -t 192.168.1.0 --ports 1-65535 -c 500 \
  --save-profile full-port-scan
```

### 10.2 プロファイルの使用

```bash
# 保存したプロファイルで実行
nelst load traffic --profile production-load-test
```

### 10.3 プロファイル管理コマンド

```bash
# プロファイル一覧
nelst profile list

# プロファイル詳細
nelst profile show production-load-test

# プロファイル削除
nelst profile delete old-profile

# 確認なしで削除
nelst profile delete old-profile --force
```

### 10.4 プロファイルのエクスポート/インポート

チームでの設定共有に便利です。

```bash
# エクスポート
nelst profile export production-load-test -o my-config.toml

# インポート
nelst profile import shared-config.toml --name imported-config
```

### 🎯 ハンズオン課題7

1. エコーサーバへの負荷テスト設定をプロファイルとして保存
2. プロファイル一覧で保存されたことを確認
3. プロファイルを使って負荷テストを実行

```bash
# 解答例
nelst load traffic -t 127.0.0.1:8080 -c 5 -d 10 --save-profile my-test
nelst profile list
nelst profile show my-test
nelst load traffic --profile my-test
```

---

## 11. レベル9: レポート出力

テスト結果を様々なフォーマットで出力・保存できます。

### 11.1 出力形式

| 形式 | 用途 |
|------|------|
| `text` | 人間が読むデフォルト出力 |
| `json` | プログラムでの処理、API連携 |
| `csv` | Excel、スプレッドシート |
| `html` | ブラウザで閲覧、レポート共有 |
| `markdown` | ドキュメント、Wiki |

### 11.2 使用例

```bash
# JSON形式で標準出力
nelst load http -u http://127.0.0.1:8080 --json

# HTMLレポートをファイルに保存
nelst load http -u http://127.0.0.1:8080 --format html --report result.html

# CSVでスキャン結果を保存
nelst scan port -t 127.0.0.1 --ports 1-1024 --format csv --report scan.csv

# Markdownでレイテンシ測定結果を保存
nelst bench latency -t 127.0.0.1:5201 --format markdown --report latency.md
```

### 11.3 設定ファイル

`~/.nelst/config.toml` または `./nelst.toml` にデフォルト設定を記述できます。

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

### 🎯 ハンズオン課題8

1. ポートスキャンを実行し、結果をHTML形式で保存
2. ブラウザで結果を確認

```bash
# 解答例
nelst scan port -t 127.0.0.1 --ports 1-1024 --format html --report scan-report.html
# ブラウザで scan-report.html を開く
```

---

## 12. 機能一覧クイックリファレンス

### 🖥️ サーバ（`nelst server`）

| コマンド | 説明 |
|---------|------|
| `nelst server echo -b 0.0.0.0:8080` | エコーサーバ |
| `nelst server echo -b 0.0.0.0:8080 -p udp` | UDPエコーサーバ |
| `nelst server sink -b 0.0.0.0:8080` | シンクサーバ |
| `nelst server flood -b 0.0.0.0:8080 -s 4096` | フラッドサーバ |
| `nelst server http -b 0.0.0.0:8080` | HTTPサーバ |
| `nelst server http --delay 50 --error-rate 0.1` | 遅延+エラーシミュレート |

### 🚀 負荷テスト（`nelst load`）

| コマンド | 説明 |
|---------|------|
| `nelst load traffic -t HOST:PORT -d 60` | トラフィック負荷テスト |
| `nelst load traffic -t HOST:PORT -c 10 -s 4096` | 10並列、4KBパケット |
| `nelst load connection -t HOST:PORT -n 1000` | 1000コネクション確立 |
| `nelst load http -u URL -d 60 -c 10` | HTTP負荷テスト |
| `nelst load http -u URL -X POST -b '{"key":"val"}'` | POSTリクエスト |

### 🔍 スキャン（`nelst scan`）

| コマンド | 説明 |
|---------|------|
| `nelst scan port -t IP --ports 1-1024` | TCP Connectスキャン |
| `sudo nelst scan port -t IP -m syn` | SYNスキャン |
| `nelst scan port -t IP -m udp --ports 53,123` | UDPスキャン |
| `nelst scan port -t IP --service-detection` | サービス検出 |
| `nelst scan port -t IP --ssl-check` | SSL/TLS検査 |

### 📡 診断（`nelst diag`）

| コマンド | 説明 |
|---------|------|
| `sudo nelst diag ping -t HOST -c 5` | ICMP ping |
| `nelst diag ping -t HOST --tcp --port 443` | TCP ping |
| `nelst diag trace -t HOST` | Traceroute |
| `nelst diag dns -t DOMAIN` | DNS解決 |
| `nelst diag dns -t DOMAIN --record-type all` | 全レコードタイプ |
| `sudo nelst diag mtu -t HOST` | MTU探索 |

### 📈 測定（`nelst bench`）

| コマンド | 説明 |
|---------|------|
| `nelst bench bandwidth --server` | 帯域幅測定サーバ |
| `nelst bench bandwidth -t HOST:5201 -d 10` | 帯域幅測定クライアント |
| `nelst bench latency -t HOST:5201 -d 60` | レイテンシ測定 |
| `nelst bench latency -t HOST:5201 --histogram` | ヒストグラム付き |

### 📁 プロファイル（`nelst profile`）

| コマンド | 説明 |
|---------|------|
| `--save-profile NAME` | プロファイル保存（任意のコマンドに追加） |
| `--profile NAME` | プロファイル使用（任意のコマンドに追加） |
| `nelst profile list` | 一覧表示 |
| `nelst profile show NAME` | 詳細表示 |
| `nelst profile delete NAME` | 削除 |
| `nelst profile export NAME -o FILE` | エクスポート |
| `nelst profile import FILE` | インポート |

### 📊 出力オプション

| オプション | 説明 |
|-----------|------|
| `--json` | JSON形式で出力 |
| `--format json\|csv\|html\|markdown\|text` | 出力形式指定 |
| `--report FILE` | ファイルに保存 |
| `-v, --verbose` | 詳細ログ |
| `-q, --quiet` | 最小出力 |

---

## 🎓 修了チェックリスト

以下の項目をすべて完了したら、NeLSTの基本機能をマスターしたことになります：

- [ ] エコーサーバを起動できる
- [ ] トラフィック負荷テストを実行できる
- [ ] HTTP負荷テストでPOSTリクエストを送信できる
- [ ] ポートスキャンでオープンポートを検出できる
- [ ] サービス検出とSSL検査を実行できる
- [ ] Ping、Traceroute、DNS解決を実行できる
- [ ] 帯域幅とレイテンシを測定できる
- [ ] プロファイルを保存・使用できる
- [ ] HTMLレポートを出力できる

---

## 📖 関連ドキュメント

- [README.md](../README.md) - プロジェクト概要
- [DESIGN.md](DESIGN.md) - 詳細設計書
- [PLAN.md](PLAN.md) - 実装計画

---

Happy Testing! 🚀

# NeLST 設計書

ネットワークの負荷テストとセキュリティテストを行うCLIツールの設計ドキュメント。

---

## 1. コマンド体系

```
nelst <COMMAND> [OPTIONS]

COMMANDS:
    load        負荷テスト（トラフィック/コネクション/HTTP）
    scan        セキュリティスキャン（ポート/SSL/TLS）
    diag        ネットワーク診断（ping/traceroute/DNS）
    bench       帯域幅・レイテンシ測定
    server      テスト用サーバ起動
    report      テスト結果のレポート生成
    profile     プロファイル管理
    help        ヘルプ表示

GLOBAL OPTIONS:
    --config <FILE>         設定ファイル指定
    --profile <NAME>        プロファイル使用
    --save-profile <NAME>   現在の設定をプロファイルとして保存
    --format <FORMAT>       出力形式 [json|csv|html|markdown|text]
    --report <FILE>         結果をファイルに保存
    --quiet                 出力を最小限に
    --json                  JSON形式で出力
```

---

## 2. 負荷テスト (`load`)

### 2.1 概要

```bash
nelst load <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    traffic     データ送受信の負荷テスト
    connection  大量コネクション確立テスト
    http        HTTP/HTTPS負荷テスト
```

### 2.2 共通オプション

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target <HOST:PORT>` | `-t` | ターゲットアドレス | (必須) |
| `--protocol <tcp\|udp>` | `-p` | プロトコル | tcp |
| `--duration <SECONDS>` | `-d` | テスト継続時間 | 60 |
| `--concurrency <NUM>` | `-c` | 同時接続数 | 1 |
| `--rate <NUM>` | `-r` | 毎秒リクエスト数 | unlimited |
| `--output <FILE>` | `-o` | 結果出力ファイル | - |
| `--verbose` | `-v` | 詳細ログ出力 | false |

### 2.3 トラフィック負荷テスト (`load traffic`)

ターゲットへ指定したデータサイズのパケットを送信し続ける。

```bash
nelst load traffic [OPTIONS]

OPTIONS:
    -s, --size <BYTES>           パケットサイズ [default: 1024]
    -m, --mode <send|echo|recv>  動作モード
                                   send: 送信のみ
                                   echo: エコーサーバへ送受信
                                   recv: 受信のみ（サーバモード）
    --payload <FILE|STRING>      カスタムペイロード
```

#### 使用例

```bash
# TCPでエコーサーバへ10秒間負荷テスト
nelst load traffic -t 192.168.1.100:8080 -d 10 -s 4096 -m echo

# UDPで100並列、送信のみ
nelst load traffic -t 192.168.1.100:5000 -p udp -c 100 -m send
```

### 2.4 コネクション負荷テスト (`load connection`)

大量のTCPコネクションを確立し、サーバのコネクション処理能力をテストする。

```bash
nelst load connection [OPTIONS]

OPTIONS:
    -n, --count <NUM>            確立するコネクション総数 [default: 1000]
    --keep-alive                 コネクションを維持する
    --timeout <MS>               コネクションタイムアウト [default: 5000]
```

#### 使用例

```bash
# 10000コネクションを確立（C10K問題テスト）
nelst load connection -t 192.168.1.100:8080 -n 10000 -c 100

# コネクション維持テスト
nelst load connection -t 192.168.1.100:8080 -n 5000 --keep-alive
```

### 2.5 HTTP負荷テスト (`load http`)

HTTP/HTTPSエンドポイントに対する負荷テスト。REST APIテストに最適。

```bash
nelst load http [OPTIONS]

OPTIONS:
    -u, --url <URL>              ターゲットURL (必須)
    -X, --method <METHOD>        HTTPメソッド [default: GET]
    -H, --header <KEY:VALUE>     カスタムヘッダー（複数指定可）
    -b, --body <DATA|@FILE>      リクエストボディ
    --follow-redirects           リダイレクトを追跡
    --insecure                   SSL証明書検証をスキップ
    --http2                      HTTP/2を優先使用
    --timeout <MS>               リクエストタイムアウト [default: 30000]
```

#### 使用例

```bash
# GETリクエストで負荷テスト
nelst load http -u https://api.example.com/users -c 50 -d 30

# POSTリクエスト（JSONボディ）
nelst load http -u https://api.example.com/users \
    -X POST \
    -H "Content-Type: application/json" \
    -b '{"name": "test"}' \
    -c 20 -d 60

# 認証ヘッダー付き
nelst load http -u https://api.example.com/protected \
    -H "Authorization: Bearer token123" \
    -c 10

# ファイルからリクエストボディを読み込み
nelst load http -u https://api.example.com/data -X POST -b @request.json

# レート制限付き（100 req/s）、結果をファイルに保存
nelst load http -u https://api.example.com -r 100 -o result.json

# HTTP/2を優先使用
nelst load http -u https://example.com --http2
```

### 2.6 バッチモード

複数ターゲットへの一括テスト実行。

```bash
nelst load traffic --batch <TARGETS_FILE> [OPTIONS]
```

**targets.txt の形式:**
```
192.168.1.100:8080
192.168.1.101:8080
192.168.1.102:9000
```

---

## 3. セキュリティスキャン (`scan`)

### 3.1 概要

```bash
nelst scan <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    port        ポートスキャン
    ssl         SSL/TLS検査
    service     サービス検出・バナー取得
```

### 3.2 共通オプション

| オプション | 短縮 | 説明 | デフォルト |
|-----------|------|------|-----------|
| `--target <HOST>` | `-t` | ターゲットホスト | (必須) |
| `--ports <RANGE>` | - | ポート範囲 | 1-1024 |
| `--concurrency <NUM>` | `-c` | 並列スキャン数 | 100 |
| `--timeout <MS>` | - | タイムアウト | 1000 |
| `--output <FILE>` | `-o` | 結果出力ファイル | - |

### 3.3 ポートスキャン (`scan port`)

```bash
nelst scan port [OPTIONS]

OPTIONS:
    -m, --method <METHOD>        スキャン手法
                                   tcp:  TCP Connect スキャン (default)
                                   syn:  SYN スキャン (要root)
                                   fin:  FIN スキャン (要root)
                                   xmas: Xmas スキャン (要root)
                                   null: NULL スキャン (要root)
                                   udp:  UDP スキャン
    --top-ports <NUM>            よく使われるポート上位N件のみ
    --service-detection          サービス検出を有効化
```

#### 使用例

```bash
# TCPコネクトスキャン（全ポート）
nelst scan port -t 192.168.1.100 --ports 1-65535

# SYNスキャン（ステルス）
sudo nelst scan port -t 192.168.1.100 -m syn --top-ports 1000

# UDP + サービス検出
nelst scan port -t 192.168.1.100 -m udp --service-detection
```

### 3.4 スキャン手法詳細

#### TCP Connect スキャン
通常のTCP 3ウェイハンドシェイクを完了させる。
root権限不要だが、ログに残りやすい。

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    loop Port
        C ->> S: SYN
        S -->> C: SYN/ACK (open) or RST (closed)
        C ->> S: ACK
        C ->> S: RST (切断)
    end
```

#### SYN スキャン (Half-open)
SYNパケットのみ送信し、SYN/ACKを受信したらRSTで切断。
コネクションを完了しないためステルス性が高い。

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    C ->> S: SYN
    alt Port Open
        S -->> C: SYN/ACK
        C ->> S: RST
    else Port Closed
        S -->> C: RST
    end
```

#### FIN スキャン
FINパケットを送信。クローズドポートはRSTを返し、オープンポートは無応答。

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    C ->> S: FIN
    alt Port Closed
        S -->> C: RST
    else Port Open
        Note right of S: 無応答
    end
```

#### Xmas スキャン
FIN + URG + PSH フラグを設定したパケットを送信。

#### NULL スキャン
フラグなしのパケットを送信。

#### UDP スキャン
UDPパケットを送信し、ICMP Port Unreachableの有無で判定。

### 3.5 SSL/TLS検査 (`scan ssl`)

SSL/TLS設定のセキュリティを検査する。

```bash
nelst scan ssl [OPTIONS]

OPTIONS:
    -t, --target <HOST:PORT>     ターゲット (必須)
    --check-cert                 証明書の有効性を検証
    --check-chain                証明書チェーンを検証
    --check-ciphers              暗号スイートを検査
    --check-protocols            対応プロトコルを検査
    --check-vulnerabilities      既知の脆弱性をチェック (POODLE, BEAST, etc.)
    --all                        すべての検査を実行
```

#### 使用例

```bash
# 総合SSL検査
nelst scan ssl -t example.com:443 --all

# 証明書のみ検証
nelst scan ssl -t example.com:443 --check-cert --check-chain
```

#### 出力例

```
NeLST - SSL/TLS Scanner
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Target: example.com:443

━━━━━━━━━━━ CERTIFICATE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Subject:      CN=example.com
  Issuer:       Let's Encrypt Authority X3
  Valid From:   2025-01-01
  Valid Until:  2025-03-31
  Days Left:    109 ✓
  
  Signature:    SHA256withRSA ✓
  Key Size:     2048 bit ✓

━━━━━━━━━━━ PROTOCOLS ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  TLS 1.3    ✓ Supported
  TLS 1.2    ✓ Supported
  TLS 1.1    ✗ Disabled (Good)
  TLS 1.0    ✗ Disabled (Good)
  SSLv3      ✗ Disabled (Good)

━━━━━━━━━━━ VULNERABILITIES ━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Heartbleed    ✓ Not Vulnerable
  POODLE        ✓ Not Vulnerable
  BEAST         ✓ Not Vulnerable
  CRIME         ✓ Not Vulnerable

Grade: A
```

### 3.6 サービス検出 (`scan service`)

オープンポートで動作しているサービスを特定する。

```bash
nelst scan service [OPTIONS]

OPTIONS:
    -t, --target <HOST>          ターゲットホスト (必須)
    --ports <RANGE>              対象ポート [default: detected open ports]
    --grab-banner                バナー取得を有効化
    --version-detection          バージョン検出を試行
    --aggressive                 より詳細な検出（時間がかかる）
```

#### 使用例

```bash
# バナー取得
nelst scan service -t 192.168.1.100 --ports 22,80,443 --grab-banner

# バージョン検出付き
nelst scan service -t 192.168.1.100 --version-detection
```

#### 出力例

```
NeLST - Service Detection
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Target: 192.168.1.100

  PORT      SERVICE    VERSION                    BANNER
  22/tcp    ssh        OpenSSH 8.9p1              SSH-2.0-OpenSSH_8.9p1
  80/tcp    http       nginx 1.24.0               Server: nginx/1.24.0
  443/tcp   https      nginx 1.24.0               -
  3306/tcp  mysql      MySQL 8.0.35               5.7.42-MySQL Community
```

### 3.7 スキャン結果の比較（diff）

前回のスキャン結果と比較して変化を検出する。

```bash
nelst scan port -t 192.168.1.100 --diff <PREVIOUS_RESULT>
```

#### 出力例

```
━━━━━━━━━━━ CHANGES DETECTED ━━━━━━━━━━━━━━━━━━━━━━━━━

  [+] 8080/tcp    OPENED    (was: closed)
  [-] 21/tcp     CLOSED    (was: open)
  [~] 22/tcp     ssh → dropbear (service changed)
```

---

## 4. ネットワーク診断 (`diag`)

基本的なネットワーク診断機能を提供。

```bash
nelst diag <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    ping        ICMP/TCP pingテスト
    trace       経路追跡（traceroute）
    dns         DNS解決テスト
    mtu         MTU探索
```

### 4.1 Ping (`diag ping`)

```bash
nelst diag ping [OPTIONS]

OPTIONS:
    -t, --target <HOST>          ターゲット (必須)
    -c, --count <NUM>            送信回数 [default: 4]
    -i, --interval <MS>          送信間隔 [default: 1000]
    --tcp                        TCP pingを使用（ICMP不可時）
    --port <PORT>                TCPポート（--tcp使用時）[default: 80]
```

#### 使用例

```bash
# 通常のping
nelst diag ping -t 192.168.1.1 -c 10

# TCP ping（ファイアウォール越し）
nelst diag ping -t example.com --tcp --port 443
```

### 4.2 Traceroute (`diag trace`)

```bash
nelst diag trace [OPTIONS]

OPTIONS:
    -t, --target <HOST>          ターゲット (必須)
    --max-hops <NUM>             最大ホップ数 [default: 30]
    --tcp                        TCPを使用
    --udp                        UDPを使用（デフォルト）
    --icmp                       ICMPを使用
```

### 4.3 DNS解決 (`diag dns`)

```bash
nelst diag dns [OPTIONS]

OPTIONS:
    -t, --target <DOMAIN>        対象ドメイン (必須)
    --record-type <TYPE>         レコードタイプ [a|aaaa|mx|txt|ns|cname|soa|ptr|all]
    -s, --server <IP>            DNSサーバ指定
    --tcp                        TCP経由で問い合わせ
    --timeout <MS>               タイムアウト [default: 5000]
```

#### 使用例

```bash
# Aレコードを検索
nelst diag dns -t google.com

# MXレコードを検索
nelst diag dns -t google.com --record-type mx

# すべてのレコードを取得
nelst diag dns -t google.com --record-type all

# 特定のDNSサーバを使用
nelst diag dns -t google.com -s 8.8.8.8
```

### 4.4 MTU探索 (`diag mtu`)

```bash
nelst diag mtu [OPTIONS]

OPTIONS:
    -t, --target <HOST>          ターゲット (必須)
    --min-mtu <BYTES>            最小MTU [default: 576]
    --max-mtu <BYTES>            最大MTU [default: 1500]
    --timeout <MS>               タイムアウト [default: 1000]
```

#### 使用例

```bash
# Path MTU探索
sudo nelst diag mtu -t google.com

# 探索範囲を指定
sudo nelst diag mtu -t 192.168.1.1 --min-mtu 576 --max-mtu 9000
```

---

## 5. 帯域幅測定 (`bench`)

ネットワーク帯域幅とレイテンシを測定。

```bash
nelst bench <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    bandwidth   帯域幅測定
    latency     レイテンシ測定
```

### 5.1 帯域幅測定 (`bench bandwidth`)

```bash
nelst bench bandwidth [OPTIONS]

OPTIONS:
    -t, --target <HOST:PORT>     ターゲット（サーバモード起動が必要）
    -d, --duration <SECONDS>     測定時間 [default: 10]
    --direction <up|down|both>   測定方向 [default: both]
    -p, --parallel <NUM>         並列ストリーム数 [default: 1]
```

#### 使用例

```bash
# サーバ側
nelst bench bandwidth --server -b 0.0.0.0:5201

# クライアント側
nelst bench bandwidth -t 192.168.1.100:5201 -d 30 --direction both
```

#### 出力例

```
NeLST - Bandwidth Test
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Connecting to 192.168.1.100:5201...
Duration: 10s

━━━━━━━━━━━ UPLOAD ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  [========================================] 100%
  
  Average:    942.5 Mbps
  Peak:       987.2 Mbps
  Jitter:     2.3 ms

━━━━━━━━━━━ DOWNLOAD ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  [========================================] 100%
  
  Average:    956.8 Mbps
  Peak:       991.4 Mbps
  Jitter:     1.8 ms
```

### 5.2 レイテンシ測定 (`bench latency`)

継続的なレイテンシ監視。

```bash
nelst bench latency [OPTIONS]

OPTIONS:
    -t, --target <HOST:PORT>     ターゲット (必須)
    -c, --count <NUM>            測定回数 [default: 100]
    -d, --duration <SECONDS>     測定時間 [default: 60]
    -i, --interval <MS>          測定間隔 [default: 100]
    --histogram                  ヒストグラム表示
    --server                     サーバモードで起動
    -b, --bind <HOST:PORT>       バインドアドレス [default: 0.0.0.0:5201]
```

#### 使用例

```bash
# サーバ側
nelst bench latency --server -b 0.0.0.0:5201

# クライアント側（100回測定）
nelst bench latency -t 192.168.1.100:5201 -c 100

# ヒストグラム付きで測定
nelst bench latency -t 192.168.1.100:5201 -d 30 --histogram
```

---

## 6. サーバモード (`server`)

負荷テストのターゲットとして使用するサーバを起動する。

```bash
nelst server <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
    echo        エコーサーバ（受信データをそのまま返す）
    sink        シンクサーバ（受信のみ、応答なし）
    flood       フラッドサーバ（指定サイズのデータを送り続ける）
    http        HTTPテストサーバ

OPTIONS (echo/sink/flood):
    -b, --bind <HOST:PORT>       バインドアドレス [default: 0.0.0.0:8080]
    -p, --protocol <tcp|udp>     プロトコル [default: tcp]
    -s, --size <BYTES>           応答サイズ (floodで使用)

OPTIONS (http):
    -b, --bind <HOST:PORT>       バインドアドレス [default: 0.0.0.0:8080]
    --body <STRING>              レスポンスボディ [default: OK]
    --status <CODE>              レスポンスステータスコード [default: 200]
    --delay <MS>                 レスポンス遅延（ミリ秒） [default: 0]
    --error-rate <0.0-1.0>       エラー率（5xxを返す確率） [default: 0]
```

#### 使用例

```bash
# エコーサーバ起動
nelst server echo -b 0.0.0.0:8080

# UDPエコーサーバ
nelst server echo -b 0.0.0.0:8080 -p udp

# UDPシンクサーバ
nelst server sink -b 0.0.0.0:5000 -p udp

# フラッドサーバ（4KBデータを送信）
nelst server flood -b 0.0.0.0:8080 -s 4096

# HTTPテストサーバ
nelst server http -b 0.0.0.0:8080

# 遅延シミュレーション付きHTTPサーバ（100ms遅延）
nelst server http -b 0.0.0.0:8080 --delay 100

# 10%エラー率のHTTPサーバ（エラーハンドリングテスト用）
nelst server http -b 0.0.0.0:8080 --error-rate 0.1

# 帯域幅測定サーバ
nelst server bandwidth -b 0.0.0.0:5201
```

---

## 7. レポート (`report`)

テスト結果を各種フォーマットで出力する。

```bash
nelst report <INPUT_FILE> [OPTIONS]

OPTIONS:
    -f, --format <FORMAT>        出力形式 [json|csv|html|text]
    -o, --output <FILE>          出力ファイル
    --compare <FILE>             別結果ファイルと比較
```

---

## 8. 出力例

### 負荷テスト結果

```
NeLST - Network Load Test
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Target:       192.168.1.100:8080
Protocol:     TCP
Mode:         Echo
Duration:     10s
Concurrency:  10
Packet Size:  1024 bytes

Running... ████████████████████████████████████████ 100% [10s/10s]

━━━━━━━━━━━━━━━━━ RESULTS ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Total Requests:     125,432
  Successful:         125,430 (99.99%)
  Failed:             2 (0.01%)
  
  Throughput:         12,543 req/s
  Data Transferred:   122.5 MB
  
  Latency:
    Min:    0.5 ms
    Max:    45.2 ms
    Avg:    2.3 ms
    P50:    1.8 ms
    P95:    8.5 ms
    P99:    25.1 ms
```

### ポートスキャン結果

```
NeLST - Port Scanner
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Target: 192.168.1.100
Method: SYN Scan
Ports:  Top 100

Scanning... ████████████████████████████████████████ 100%

━━━━━━━━━━━━━━━━━ OPEN PORTS ━━━━━━━━━━━━━━━━━━━━━━━━━━━

  PORT      STATE    SERVICE
  22/tcp    open     ssh
  80/tcp    open     http
  443/tcp   open     https
  3306/tcp  open     mysql

Scan completed in 2.3s
```

---

## 9. プロファイル管理 (`profile`)

よく使う設定を保存・管理する。

```bash
nelst profile <SUBCOMMAND>

SUBCOMMANDS:
    save <NAME>     現在のオプションをプロファイルとして保存
    list            保存済みプロファイル一覧
    show <NAME>     プロファイル内容を表示
    delete <NAME>   プロファイルを削除
    export <NAME>   プロファイルをファイルにエクスポート
    import <FILE>   ファイルからプロファイルをインポート
```

#### 使用例

```bash
# プロファイル保存
nelst load traffic -t 192.168.1.100:8080 -c 50 -d 60 --save-profile prod-load-test

# プロファイル使用
nelst load traffic --profile prod-load-test

# プロファイル一覧
nelst profile list

# プロファイルエクスポート（チーム共有用）
nelst profile export prod-load-test > prod-load-test.toml
```

#### プロファイル保存先

```
~/.nelst/profiles/
├── prod-load-test.toml
├── staging-scan.toml
└── local-dev.toml
```

---

## 10. 技術設計

### 10.1 使用クレート

| クレート | 用途 |
|---------|------|
| `clap` | CLI引数パーサー（サブコマンド対応） |
| `tokio` | 非同期ランタイム |
| `mio` | 低レベルI/O（継続使用） |
| `pnet` | Raw socketパケット操作 |
| `socket2` | ソケットオプション制御 |
| `rustls` / `native-tls` | SSL/TLS処理 |
| `trust-dns-resolver` | DNS解決 |
| `reqwest` | HTTP/HTTPSクライアント |
| `hyper` | HTTPサーバ |
| `indicatif` | プログレスバー表示 |
| `serde` / `serde_json` | シリアライズ/JSON出力 |
| `log` / `tracing` | ロギング・トレーシング |
| `chrono` | タイムスタンプ |
| `dirs` | プロファイル保存先 |

### 10.2 モジュール構成

```
src/
├── main.rs              # エントリポイント、CLI定義
├── cli/
│   ├── mod.rs           # CLIパーサー
│   ├── load.rs          # loadサブコマンド
│   ├── scan.rs          # scanサブコマンド
│   ├── diag.rs          # diagサブコマンド
│   ├── bench.rs         # benchサブコマンド
│   ├── server.rs        # serverサブコマンド
│   └── profile.rs       # profileサブコマンド
├── load/
│   ├── mod.rs
│   ├── traffic.rs       # トラフィック負荷テスト
│   ├── connection.rs    # コネクション負荷テスト
│   └── http.rs          # HTTP負荷テスト
├── scan/
│   ├── mod.rs
│   ├── tcp_connect.rs   # TCP Connectスキャン
│   ├── syn.rs           # SYNスキャン
│   ├── fin.rs           # FINスキャン
│   ├── xmas.rs          # Xmasスキャン
│   ├── null.rs          # NULLスキャン
│   ├── udp.rs           # UDPスキャン
│   ├── ssl.rs           # SSL/TLS検査
│   └── service.rs       # サービス検出
├── diag/
│   ├── mod.rs
│   ├── ping.rs          # Ping
│   ├── trace.rs         # Traceroute
│   ├── dns.rs           # DNS解決
│   └── mtu.rs           # MTU探索
├── bench/
│   ├── mod.rs
│   ├── bandwidth.rs     # 帯域幅測定
│   └── latency.rs       # レイテンシ測定
├── server/
│   ├── mod.rs
│   ├── echo.rs          # エコーサーバ
│   ├── sink.rs          # シンクサーバ
│   ├── flood.rs         # フラッドサーバ
│   ├── http.rs          # HTTPサーバ
│   └── bandwidth.rs     # 帯域幅測定サーバ
├── report/
│   ├── mod.rs
│   ├── formatter.rs     # 出力フォーマッタ
│   └── diff.rs          # 差分比較
├── profile/
│   ├── mod.rs
│   └── manager.rs       # プロファイル管理
└── common/
    ├── mod.rs
    ├── stats.rs         # 統計収集
    ├── config.rs        # 設定管理
    └── output.rs        # 出力ユーティリティ
```

### 10.3 設計方針

- **CLI優先**: 設定ファイルよりCLI引数を優先。`--config` で設定ファイル指定も可能
- **非同期処理**: `tokio` ベースの非同期I/O
- **プログレス表示**: リアルタイムで進捗を可視化
- **構造化出力**: JSON形式で機械処理可能な出力をサポート

---

## 11. 設定ファイル

`~/.nelst/config.toml` または `./nelst.toml` でデフォルト値を設定可能。
CLI引数は設定ファイルより優先される。

```toml
[defaults]
verbose = false
timeout = 5000

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

---

## 12. エラーハンドリング

### 終了コード

| コード | 説明 |
|-------|------|
| 0 | 正常終了 |
| 1 | 一般的なエラー |
| 2 | 引数エラー |
| 3 | 接続エラー |
| 4 | 権限エラー（要root） |
| 5 | タイムアウト |

### エラーメッセージ例

```
Error: Permission denied. SYN scan requires root privileges.
Hint: Run with 'sudo nelst scan port -m syn ...'

Error: Connection refused to 192.168.1.100:8080
Hint: Verify the target is running and accessible.
```

---

## 13. 注意事項

### 法的注意

⚠️ **重要**: ポートスキャンや負荷テストは、**許可されたシステムに対してのみ**実行してください。

- 無許可でのスキャンは不正アクセス禁止法等に抵触する可能性があります
- 本番環境への負荷テストは事前に関係者と調整してください
- テスト実行前に対象システムの管理者から書面での許可を取得することを推奨します

### 技術的注意

- SYN/FIN/Xmas/NULL/UDPスキャンには **root権限** が必要です
- 一部のファイアウォールやIDS/IPSはスキャンを検知・ブロックする場合があります
- 負荷テストは対象システムおよびネットワーク経路に影響を与える可能性があります
- クラウド環境では、プロバイダのペネトレーションテストポリシーを確認してください

---

## 14. 実装状況（v0.5.0現在）

本設計書は最終目標を示しています。現在の実装状況については [PLAN.md](PLAN.md) を参照してください。

### 実装済み（Phase 0-5）

| 機能カテゴリ | 状態 | 備考 |
|-------------|------|------|
| CLI基盤 | ✅ 完了 | clap derive マクロ |
| 負荷テスト（traffic/connection） | ✅ 完了 | TCP/UDP対応 |
| HTTP負荷テスト | ✅ 完了 | GET/POST/PUT/DELETE, HTTP/2 |
| テストサーバ | ✅ 完了 | echo/sink/flood/http |
| ポートスキャン | ✅ 完了 | tcp/syn/fin/xmas/null/udp |
| サービス検出 | ✅ 完了 | `--grab-banner`, `--service-detection` |
| SSL/TLS検査 | ✅ 完了 | `--ssl-check` オプション |
| 統計・出力 | ✅ 完了 | JSON出力、ファイル出力 |
| Ping | ✅ 完了 | ICMP/TCP ping、統計表示 |
| Traceroute | ✅ 完了 | UDP/TCP/ICMPモード |
| DNS解決 | ✅ 完了 | A/AAAA/MX/TXT/NS/CNAME/SOA/PTR |
| MTU探索 | ✅ 完了 | Path MTU Discovery |
| 帯域幅測定 | ✅ 完了 | Upload/Download/Both |
| レイテンシ測定 | ✅ 完了 | ヒストグラム、異常値検出 |
| プロファイル管理 | ✅ 完了 | `--save-profile`, `--profile` |
| レポート機能 | ✅ 完了 | JSON/CSV/HTML/Markdown/Text |
| 設定ファイル | ✅ 完了 | `--config`, `~/.nelst/config.toml` |

### 設計との差異

現在の実装では、以下の点が設計と異なります：

1. **SSL/TLS検査**: 独立サブコマンド（`scan ssl`）ではなく、`scan port --ssl-check` オプションとして統合
2. **サービス検出**: 独立サブコマンド（`scan service`）ではなく、`scan port --service-detection --grab-banner` として統合
3. **レポート機能**: `report` サブコマンドではなく、`--format` と `--report` グローバルオプションとして実装

### 未実装（v0.6.0以降に予定）

- SSL/TLS脆弱性チェック・グレード評価
- スキャン結果比較（diff）
- 統合テスト（CLIレベル）
- ベンチマークテスト
- crates.io公開
- Docker イメージ

---

## 15. 将来の拡張案（v1.0.0以降）

### 拡張フェーズ1
- [ ] 分散負荷テスト（コーディネーター/ワーカーモデル）
- [ ] WebSocket負荷テスト対応
- [ ] gRPC負荷テスト対応
- [ ] リアルタイムメトリクス送信（Prometheus/InfluxDB）

### 拡張フェーズ2
- [ ] OS検出（TCP/IPスタックフィンガープリント）
- [ ] 脆弱性データベース連携（CVE検出）
- [ ] スクリプトエンジン（Lua/Rhai）
- [ ] プラグインシステム

### 拡張フェーズ3
- [ ] Web UI / ダッシュボード
- [ ] スケジュール実行（cronライク）
- [ ] アラート通知（Slack/Discord/Email）
- [ ] マルチターゲット同時テスト

---

## 16. 参照

- [tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)
- [Crate mio - Rust](https://docs.rs/mio/latest/mio/)
- [Crate clap - Rust](https://docs.rs/clap/latest/clap/)
- [Crate pnet - Rust](https://docs.rs/pnet/latest/pnet/)
- [Struct std::sync::RwLock - Rust](https://doc.rust-lang.org/std/sync/struct.RwLock.html)
- [Rust入門 Chapter 18 並列処理 - Zenn](https://zenn.dev/mebiusbox/books/22d4c1ed9b0003/viewer/98dc80)
- [Rust はどのようにして安全な並列処理を提供するのか - Qiita](https://qiita.com/nirasan/items/97263103f076bd525a7b)
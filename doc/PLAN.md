# NeLST 実装計画

DESIGN.mdに基づく段階的な実装計画。

---

## 実装フェーズ概要

| フェーズ | バージョン | 目標 | 期間目安 |
|---------|-----------|------|---------|
| 0 | v0.0.x | 基盤整備・リファクタリング | 1週間 |
| 1 | v0.1.0 | MVP（最小限の機能） | 2-3週間 |
| 2 | v0.2.0 | コア機能完成 | 2-3週間 |
| 3 | v0.3.0 | セキュリティ機能強化 | 2週間 |
| 4 | v0.4.0 | 診断・測定機能 | 2週間 |
| 5 | v0.5.0 | 運用機能・安定化 | 2週間 |

---

## フェーズ 0: 基盤整備（v0.0.x） ✅

既存コードのリファクタリングと新アーキテクチャへの移行準備。

### 0.1 プロジェクト構造の整理

- [x] 新しいモジュール構造の作成
  ```
  src/
  ├── cli/
  ├── load/
  ├── scan/
  ├── server/
  └── common/
  ```
- [x] 既存の `tcp_client.rs`, `tcp_server.rs` を新構造へ移行
- [x] `initialize/` モジュールを `common/config.rs` へ統合

### 0.2 依存関係の更新

- [x] `Cargo.toml` の依存関係を更新
  ```toml
  [dependencies]
  clap = { version = "4", features = ["derive"] }
  tokio = { version = "1", features = ["full"] }
  mio = { version = "1", features = ["os-poll", "net"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  tracing = "0.1"
  tracing-subscriber = "0.3"
  anyhow = "1"
  thiserror = "1"
  indicatif = "0.17"
  chrono = { version = "0.4", features = ["serde"] }
  dirs = "5"
  ```

### 0.3 エラーハンドリング基盤

- [x] `common/error.rs` - カスタムエラー型の定義
- [x] 終了コード（0-5）の実装
- [x] `anyhow` / `thiserror` によるエラー伝播

### 0.4 ロギング基盤

- [x] `log4rs` から `tracing` への移行
- [x] `--verbose` フラグの実装
- [x] ログレベル制御

---

## フェーズ 1: MVP（v0.1.0） ✅

最小限の動作する製品。基本的な負荷テストとTCPスキャンが可能な状態。

### 1.1 CLI基盤

**ファイル**: `src/cli/mod.rs`

- [x] `clap` derive マクロによるCLI定義
- [x] グローバルオプション（`--verbose`, `--json`, `--quiet`）
- [x] サブコマンド構造（load, scan, server, help）

```rust
#[derive(Parser)]
#[command(name = "nelst", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(long, global = true)]
    verbose: bool,
    
    #[arg(long, global = true)]
    json: bool,
}
```

### 1.2 負荷テスト - トラフィック（基本）

**ファイル**: `src/load/traffic.rs`

- [x] TCP送信のみモード（`-m send`）
- [x] エコーモード（`-m echo`）
- [x] 基本オプション
  - `-t, --target`
  - `-d, --duration`
  - `-s, --size`
  - `-c, --concurrency`
- [x] 基本統計（送信数、成功率、レイテンシ）

### 1.3 負荷テスト - コネクション（基本）

**ファイル**: `src/load/connection.rs`

- [x] TCPコネクション確立テスト
- [x] `-n, --count` オプション
- [x] `--timeout` オプション
- [x] 成功/失敗カウント

### 1.4 ポートスキャン（TCP Connect）

**ファイル**: `src/scan/tcp_connect.rs`

- [x] TCP Connectスキャン実装
- [x] ポート範囲指定（`--ports 1-1024`）
- [x] 並列スキャン（`-c, --concurrency`）
- [x] タイムアウト設定
- [x] 結果表示（open/closed/filtered）

### 1.5 テストサーバ（基本）

**ファイル**: `src/server/echo.rs`, `src/server/sink.rs`

- [x] エコーサーバ
- [x] シンクサーバ
- [x] バインドアドレス設定（`-b, --bind`）

### 1.6 出力フォーマット

**ファイル**: `src/common/output.rs`

- [x] テキスト出力（デフォルト）
- [x] JSON出力（`--json`）
- [x] プログレスバー表示（`indicatif`）

### 1.7 MVP完了条件

- [x] `nelst load traffic -t 127.0.0.1:8080 -d 10` が動作
- [x] `nelst load connection -t 127.0.0.1:8080 -n 100` が動作
- [x] `nelst scan port -t 127.0.0.1 --ports 1-1024` が動作
- [x] `nelst server echo -b 0.0.0.0:8080` が動作
- [x] 基本的なエラーハンドリングが機能

---

## フェーズ 2: コア機能完成（v0.2.0） ✅

主要機能の完成とUDP対応。

### 2.1 UDP対応

- [x] `src/load/traffic.rs` - UDP送信モード
- [x] `src/server/echo.rs` - UDPエコーサーバ
- [x] `src/server/sink.rs` - UDPシンクサーバ

### 2.2 HTTP負荷テスト

**ファイル**: `src/load/http.rs`

**依存追加**:
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "http2"] }
```

- [x] GET/POST/PUT/DELETE メソッド
- [x] カスタムヘッダー（`-H`）
- [x] リクエストボディ（`-b`）
- [x] ファイルからボディ読み込み（`-b @file`）
- [x] `--insecure` オプション
- [x] `--follow-redirects` オプション
- [x] HTTP/2サポート（`--http2`）

### 2.3 HTTPテストサーバ

**ファイル**: `src/server/http.rs`

**依存追加**:
```toml
hyper = { version = "1", features = ["full"] }
hyper-util = { version = "0.1", features = ["tokio"] }
http-body-util = "0.1"
rand = "0.8"
```

- [x] 固定レスポンスサーバ
- [x] 遅延シミュレーション（`--delay`）
- [x] エラー率設定（`--error-rate`）

### 2.4 フラッドサーバ

**ファイル**: `src/server/flood.rs`

- [x] 指定サイズのデータを送信し続ける
- [x] TCP/UDP両対応

### 2.5 統計機能強化

**ファイル**: `src/common/stats.rs`

- [x] パーセンタイル計算（P50, P95, P99）
- [ ] ヒストグラム（延期: Phase 4）
- [ ] リアルタイム統計更新（延期: Phase 4）
- [x] 結果のファイル出力（`-o, --output`）

### 2.6 レート制限

- [x] `--rate` オプション実装
- [x] トークンバケットアルゴリズム

### 2.7 バッチモード

- [ ] `--batch <FILE>` オプション（延期: Phase 3）
- [ ] ターゲットファイル読み込み（延期: Phase 3）
- [ ] 順次/並列実行オプション（延期: Phase 3）

---

## フェーズ 3: セキュリティ機能強化（v0.3.0） ✅

高度なスキャン機能の実装。

### 3.1 Raw Socket基盤

**ファイル**: `src/scan/raw_socket.rs`

**依存追加**:
```toml
pnet = "0.35"
socket2 = "0.5"
libc = "0.2"
```

- [x] Raw socket権限チェック（`check_root_privileges()`）
- [x] パケット構築ユーティリティ（`build_tcp_packet()`）
- [x] TCPチェックサム計算（`tcp_checksum()`）
- [x] CAP_NET_RAWの説明・ガイド（エラーメッセージにヒント）

### 3.2 SYNスキャン

**ファイル**: `src/scan/syn.rs`

- [x] SYNパケット構築
- [x] SYN/ACK・RST応答解析
- [x] root権限チェック・エラーメッセージ

### 3.3 その他のスキャン手法

**ファイル**: `src/scan/syn.rs`（共通実装）

- [x] FINスキャン
- [x] Xmasスキャン
- [x] NULLスキャン
- [x] 応答なし=オープンの判定ロジック

### 3.4 UDPスキャン

**ファイル**: `src/scan/udp.rs`

- [x] UDPパケット送信
- [ ] ICMP Port Unreachable検出（延期: Raw Socket必要）
- [x] タイムアウト処理

### 3.5 サービス検出

**ファイル**: `src/scan/service.rs`

- [x] バナー取得（`--grab-banner`）
- [x] サービス識別（SSH, HTTP, SMTP, FTP, MySQL, Redis等）
- [x] バージョン検出（バナーから抽出）
- [ ] サービスデータベース（JSON/TOML）（延期: Phase 4）

### 3.6 SSL/TLS検査

**ファイル**: `src/scan/ssl.rs`

**依存追加**:
```toml
rustls = { version = "0.23", features = ["ring"] }
x509-parser = "0.16"
webpki-roots = "0.26"
tokio-rustls = { version = "0.26", features = ["ring"] }
```

- [x] 証明書情報取得（Subject, Issuer, SAN, 鍵サイズ）
- [x] 有効期限チェック（`days_until_expiry`）
- [x] 対応プロトコル検査（TLSバージョン）
- [x] 暗号スイート検査
- [ ] 既知脆弱性チェック（POODLE, BEAST等）（延期: Phase 4）
- [ ] グレード評価（延期: Phase 4）

### 3.7 スキャン結果比較（diff）

- [ ] `--diff <FILE>` オプション（延期: Phase 4）
- [ ] 新規オープンポート検出（延期: Phase 4）
- [ ] クローズされたポート検出（延期: Phase 4）
- [ ] サービス変更検出（延期: Phase 4）

---

## フェーズ 4: 診断・測定機能（v0.4.0） ✅ 完了

ネットワーク診断と帯域測定。

### 4.0 Phase 2-3 からの延期項目

以下の項目はPhase 2-3で延期され、Phase 5で対応予定：

- [x] ヒストグラム表示（`src/bench/latency.rs`）
- [ ] リアルタイム統計更新（延期: Phase 5）
- [ ] バッチモード（`--batch <FILE>`）（延期: Phase 5）
- [ ] ICMP Port Unreachable検出（UDP scan）（延期: Phase 5）
- [ ] サービスデータベース（JSON/TOML）（延期: Phase 5）
- [ ] SSL/TLS 既知脆弱性チェック（POODLE, BEAST等）（延期: Phase 5）
- [ ] SSL/TLS グレード評価（延期: Phase 5）
- [ ] スキャン結果比較（`--diff <FILE>`）（延期: Phase 5）

### 4.1 Ping ✅

**ファイル**: `src/diag/ping.rs`

**依存追加**:
```toml
surge-ping = "0.8"
```

- [x] ICMP Echo Request/Reply
- [x] TCP ping（ICMP不可時の代替）
- [x] 統計表示（min/max/avg/stddev）
- [x] JSON出力対応

### 4.2 Traceroute ✅

**ファイル**: `src/diag/trace.rs`

- [x] TTL増加によるホップ検出
- [x] UDP/TCP/ICMPモード
- [x] ホップごとのレイテンシ表示
- [x] JSON出力対応

### 4.3 DNS解決 ✅

**ファイル**: `src/diag/dns.rs`

**依存追加**:
```toml
hickory-resolver = "0.25"  # 旧trust-dns-resolver
```

- [x] A/AAAA/MX/TXT/NS/CNAME/SOA/PTR レコード
- [x] カスタムDNSサーバ指定
- [x] TCP/UDP切り替え
- [x] 解決時間測定
- [x] JSON出力対応

### 4.4 MTU探索 ✅

**ファイル**: `src/diag/mtu.rs`

- [x] Path MTU Discovery
- [x] DF（Don't Fragment）フラグ設定（Linux）
- [x] 二分探索による最適MTU検出
- [x] JSON出力対応

### 4.5 帯域幅測定 ✅

**ファイル**: `src/bench/bandwidth.rs`

- [x] 帯域測定サーバ
- [x] 帯域測定クライアント
- [x] Upload/Download/Both測定
- [x] 並列ストリーム対応
- [x] JSON出力対応
- [ ] ジッター計算（延期: Phase 5）

### 4.6 レイテンシ測定 ✅

**ファイル**: `src/bench/latency.rs`

- [x] 継続的なレイテンシ測定
- [x] ヒストグラム表示
- [x] 異常値検出（IQR法）
- [x] JSON出力対応

### 4.7 テストカバレッジ ✅

- [x] diag/ping.rs: 9テスト
- [x] diag/trace.rs: 7テスト
- [x] diag/dns.rs: 13テスト
- [x] diag/mtu.rs: 10テスト
- [x] bench/bandwidth.rs: 5テスト
- [x] bench/latency.rs: 17テスト
- [x] cli/diag.rs: 15テスト
- [x] cli/bench.rs: 11テスト
- [x] 全154テストがパス

---

## フェーズ 5: 運用機能・安定化（v0.5.0）

運用に必要な機能と品質向上。

### 5.1 プロファイル管理

**ファイル**: `src/profile/manager.rs`

- [ ] プロファイル保存（`--save-profile`）
- [ ] プロファイル読み込み（`--profile`）
- [ ] プロファイル一覧/表示/削除
- [ ] エクスポート/インポート

### 5.2 設定ファイル

**ファイル**: `src/common/config.rs`

- [ ] `~/.nelst/config.toml` 読み込み
- [ ] `./nelst.toml` 読み込み（優先）
- [ ] CLI引数 > 設定ファイルの優先順位
- [ ] `--config` オプション

### 5.3 レポート機能

**ファイル**: `src/report/formatter.rs`

- [ ] JSON出力
- [ ] CSV出力
- [ ] HTML出力
- [ ] Markdown出力
- [ ] 結果比較（`--compare`）
- [ ] トレンド分析（`--trend`）

### 5.4 ドキュメント

- [ ] README.md 更新
- [ ] インストール手順
- [ ] 使用例（Examples）
- [ ] マニュアルページ（man page）
- [ ] `--help` メッセージの充実

### 5.5 テスト

- [ ] ユニットテスト（各モジュール）
- [ ] 統合テスト（CLIレベル）
- [ ] ベンチマークテスト
- [ ] CI/CD設定（GitHub Actions）

### 5.6 パッケージング

- [ ] `cargo install` 対応
- [ ] crates.io公開準備
- [ ] バイナリリリース（Linux/macOS/Windows）
- [ ] Docker イメージ

---

## タスク依存関係

```mermaid
graph TD
    A[フェーズ0: 基盤整備] --> B[フェーズ1: MVP]
    B --> C[フェーズ2: コア機能]
    B --> D[フェーズ3: セキュリティ]
    C --> E[フェーズ4: 診断・測定]
    D --> E
    C --> F[フェーズ5: 運用機能]
    E --> F
```

### クリティカルパス

1. CLI基盤 → 全機能
2. 統計基盤 → 負荷テスト結果表示
3. Raw Socket基盤 → SYN/FIN/Xmas/NULLスキャン
4. 出力フォーマット → レポート機能

---

## 実装優先順位

### 高優先度（Must Have）

1. CLI基盤（clap）
2. TCPトラフィック負荷テスト
3. TCP Connectスキャン
4. エコーサーバ
5. 基本統計
6. JSON出力

### 中優先度（Should Have）

7. HTTP負荷テスト
8. SYNスキャン
9. SSL/TLS検査
10. サービス検出
11. プロファイル管理
12. UDP対応

### 低優先度（Nice to Have）

13. 診断機能（ping/traceroute/DNS）
14. 帯域測定
15. FIN/Xmas/NULLスキャン
16. HTML/Markdownレポート
17. トレンド分析

---

## 品質基準

### コード品質

- [x] `cargo clippy` 警告なし
- [x] `cargo fmt` 適用
- [ ] 全公開APIにドキュメントコメント
- [ ] エラーメッセージは日本語/英語対応可能な設計

### テストカバレッジ

- [ ] ユニットテスト: 70%以上
- [ ] 統合テスト: 主要ユースケースをカバー

### パフォーマンス

- [ ] 10,000 req/s 以上のスループット（負荷テストクライアント）
- [ ] 1,000 ports/s 以上のスキャン速度
- [ ] メモリ使用量 < 100MB（通常使用時）

---

## リスクと対策

| リスク | 影響 | 対策 |
|-------|------|------|
| Raw socket権限 | SYN等のスキャンが動作しない | CAP_NET_RAW設定ガイドを提供 |
| プラットフォーム差異 | macOS/Windows で動作しない | 条件付きコンパイル、CI で複数OS テスト |
| 既存コードとの互換性 | リファクタリングで機能喪失 | 既存テストの移行、E2Eテスト |
| 依存クレートの脆弱性 | セキュリティ問題 | `cargo audit` 定期実行 |

---

## 次のアクション

1. [x] フェーズ0のタスクを開始
2. [x] `Cargo.toml` 依存関係の更新
3. [x] 新しいディレクトリ構造の作成
4. [x] CLI基盤の実装（clap）
5. [x] フェーズ1 MVPの実装
6. [x] フェーズ2 コア機能の実装
7. [x] フェーズ3 セキュリティ機能の実装
8. [x] フェーズ4 診断・測定機能の実装
9. [ ] フェーズ5 運用機能・安定化を開始

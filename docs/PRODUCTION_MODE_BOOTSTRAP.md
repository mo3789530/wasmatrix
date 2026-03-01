# Production Mode Bootstrap Rules (P0-6)

`production-foundation` の P0-6 として、Control Plane の起動設定を `production_config` フィーチャーに集約しました。

## 変更点

- `CONTROL_PLANE_MODE` (`development` / `production`) を導入
- `production` モードでは以下を必須化
  - `USE_ETCD=true`
  - `ETCD_ENDPOINTS` が有効で、etcd 接続/検証に成功すること
  - 外部 API 認証として以下いずれか
    - `EXTERNAL_API_JWT_SECRET`
    - `EXTERNAL_API_MTLS_PRINCIPALS`
- `REST_API_TLS_ENABLED=true` の場合
  - `REST_API_TLS_CERT_PATH`
  - `REST_API_TLS_KEY_PATH`
  を必須化
- `EXTERNAL_API_REQUIRE_MTLS=true` の場合
  - `EXTERNAL_API_MTLS_CA_PATH`
  を必須化

## 主要設定

- アドレス
  - `CONTROL_PLANE_ADDR` (default: `127.0.0.1:50051`)
  - `METRICS_ADDR` (default: `127.0.0.1:9100`)
  - `REST_API_ADDR` (default: `127.0.0.1:8080`)
- リーダー選出
  - `LEADER_ELECTION_TTL_SECS` (default: `10`)
  - `LEADER_ELECTION_RENEW_INTERVAL_MS` (default: `3000`)
  - `renew interval < ttl` を検証

## development モードと production モードの違い

- development
  - etcd 未設定でも起動可能
  - etcd 検証失敗時は warning ログを出して継続
- production
  - durable metadata store(etcd) が必須
  - etcd 未設定/接続失敗/検証失敗時は起動失敗
  - 認証設定不足時は起動失敗

## 実装ファイル

- `crates/wasmatrix-control-plane/src/features/production_config/`
  - `repo`: 環境変数アクセス
  - `service`: 構成値パースと production 制約検証
  - `controller`: 起動側からの読み出し
- `crates/wasmatrix-control-plane/src/main.rs`
  - 起動時に `ProductionConfigController` を使うよう変更

# Production Foundation REST API

2026-03-01

`P0-4` と `P0-5` の初期実装として、`wasmatrix-control-plane` に `external_api` feature を追加した。

変更内容:

- `REST_API_ADDR` で Axum ベースの外部 REST API を起動
- 追加ルート:
  - `POST /v1/instances`
  - `GET /v1/instances`
  - `GET /v1/instances/{id}`
  - `POST /v1/instances/{id}/stop`
  - `POST /v1/instances/{id}/capabilities`
  - `DELETE /v1/instances/{id}/capabilities/{capability_id}`
  - `POST /v1/capabilities/invoke`
  - `GET /v1/healthz`
  - `GET /v1/leader`
- JWT Bearer 認証を追加
  - `EXTERNAL_API_JWT_SECRET` を使って HS256 を検証
  - 任意で `EXTERNAL_API_JWT_ISSUER` と `EXTERNAL_API_JWT_AUDIENCE` を検証
- mTLS 相当の主体マッピングを追加
  - `x-mtls-subject` ヘッダーを `EXTERNAL_API_MTLS_PRINCIPALS` に照合
  - 形式: `subject|role1+role2|tenant`, 複数は `,` 区切り
- RBAC を追加
  - 読み取り: `instance.read`
  - ライフサイクル変更: `instance.admin`
  - capability invoke: `capability.invoke`
- 書き込み系 REST リクエストで監査ログを出力

補足:

- インスタンス作成と invoke は既存の `node_routing` を使って実行する
- capability assignment のメタデータは control-plane の共有状態に保持する
- 本実装は `P0-6` の本番設定厳格化前なので、環境変数未設定時は JWT 認証を拒否するが、起動自体は継続する

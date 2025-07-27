# PolyTorus（ポリトーラス）

> 🇬🇧 This is a Japanese translation of the [original English README](README.md).

📘 他の言語: [English](README.md)

PolyTorusは、ポスト量子時代に対応した革新的な**モジュール型ブロックチェーンプラットフォーム**です。

柔軟な暗号方式、カスタマイズ可能なアーキテクチャ、および完全なレイヤー分離を特徴とし、分散アプリケーションの次世代構築を可能にします。

---

## 🚀 クイックスタート

以下の手順でPolyTorusをビルド・実行できます：

```bash
git clone https://github.com/PolyTorus/polytorus.git
cd polytorus
cargo build --release ```

🛠 必要環境
Rust 1.70以降（推奨：最新の安定版）

Cargo（Rustに同梱）

Git

Linux、macOS、またはWSL2を使ったWindows

🔧 主な機能
Quantum-Resistantセキュリティ

ダイヤモンドIOによるスマートコントラクトの難読化

モジュール型アーキテクチャ（実行・合意・データ層の分離）

WASMスマートコントラクトサポート

マルチノードシミュレーションとネットワーク監視機能

## 🧪 Container Lab E2Eテスト環境

PolyTorusは、リアルなWebRTC P2Pネットワーキングとトランザクション伝播テストのための**完全なContainer Lab環境**を提供します。

### 🚀 Container Labクイックスタート

#### 前提条件
- **Docker**: コンテナランタイム
- **Container Lab**: ネットワークシミュレーションツール（オプション、手動Dockerアプローチも利用可能）
- **Rust 1.84+**: WASMおよびWebRTCサポート用

#### 1. テストネット用Dockerイメージのビルド
```bash
# Rust 1.84とWASMサポートを含む最適化Dockerイメージをビルド
docker build -f Dockerfile.testnet -t polytorus:testnet .
```

#### 2. 3ノードテストネットのデプロイ
```bash
# Dockerネットワークの作成
docker network create polytorus-net

# ブートストラップノード（エントリーポイント）の起動
docker run -d --name polytorus-bootstrap \
  --network polytorus-net -p 18080:8080 \
  -e NODE_ID=bootstrap-node \
  -e LISTEN_PORT=8080 \
  -e RUST_LOG=info \
  polytorus:testnet

# バリデータノード1の起動
docker run -d --name polytorus-validator1 \
  --network polytorus-net -p 18081:8080 \
  -e NODE_ID=validator-1 \
  -e LISTEN_PORT=8080 \
  -e BOOTSTRAP_PEERS=polytorus-bootstrap:8080 \
  -e RUST_LOG=info \
  polytorus:testnet

# バリデータノード2の起動
docker run -d --name polytorus-validator2 \
  --network polytorus-net -p 18082:8080 \
  -e NODE_ID=validator-2 \
  -e LISTEN_PORT=8080 \
  -e BOOTSTRAP_PEERS=polytorus-bootstrap:8080,polytorus-validator1:8080 \
  -e RUST_LOG=info \
  polytorus:testnet
```

#### 3. ネットワークデプロイの確認
```bash
# 実行中のコンテナの確認
docker ps --filter "name=polytorus-"

# ネットワーク接続のテスト
docker exec polytorus-validator1 ping -c 3 polytorus-bootstrap

# ノードログの確認
docker logs polytorus-bootstrap --tail 20
```

### 🎯 手動テストコマンド

#### ブロックチェーンの初期化
```bash
# ブートストラップノードでブロックチェーンを初期化
docker exec polytorus-bootstrap polytorus start

# ブロックチェーンステータスの確認
docker exec polytorus-bootstrap polytorus status
```

#### トランザクションの送信
```bash
# テストトランザクションの送信
docker exec polytorus-bootstrap polytorus send \
  --from alice --to bob --amount 1000

# バリデータノードからの送信
docker exec polytorus-validator1 polytorus send \
  --from validator1 --to alice --amount 500
```

#### P2Pネットワーキングのテスト
```bash
# ブートストラップノードでP2Pネットワーキングを開始
docker exec -d polytorus-bootstrap polytorus start-p2p \
  --node-id bootstrap-node --listen-port 8080

# ブートストラップピアを使用してバリデータでP2Pを開始
docker exec -d polytorus-validator1 polytorus start-p2p \
  --node-id validator-1 --listen-port 8080 \
  --bootstrap-peers polytorus-bootstrap:8080
```

#### インタラクティブなノードアクセス
```bash
# ブートストラップノードシェルへのアクセス
docker exec -it polytorus-bootstrap bash

# バリデータノードシェルへのアクセス
docker exec -it polytorus-validator1 bash

# コンテナ内で直接コマンドを実行
polytorus status
polytorus send --from alice --to bob --amount 1000
```

### 🔧 自動テストスクリプト

#### ヘルパースクリプト
```bash
# 手動テストヘルパー
./scripts/manual-test.sh start          # テストネットの構築と開始
./scripts/manual-test.sh status         # ネットワークステータスの表示
./scripts/manual-test.sh test-tx        # テストトランザクションの送信
./scripts/manual-test.sh logs bootstrap # ノードログの表示
./scripts/manual-test.sh exec bootstrap # ノードシェルへのアクセス
./scripts/manual-test.sh stop           # 停止とクリーンアップ

# E2Eテストスイート（Container Lab必須）
./scripts/run-e2e-tests.sh
```

### 🌐 ネットワーク設定

#### ノード設定
| ノード | コンテナ名 | ホストポート | ノードID | 役割 | ブートストラップピア |
|--------|------------|--------------|----------|------|---------------------|
| ブートストラップ | polytorus-bootstrap | 18080 | bootstrap-node | エントリーポイント | - |
| バリデータ1 | polytorus-validator1 | 18081 | validator-1 | バリデータ | bootstrap:8080 |
| バリデータ2 | polytorus-validator2 | 18082 | validator-2 | バリデータ | bootstrap:8080,validator1:8080 |

#### 環境変数
```bash
NODE_ID=<固有ノード識別子>          # ノード識別
LISTEN_PORT=8080                    # P2Pリスニングポート
BOOTSTRAP_PEERS=<カンマ区切り>      # ブートストラップピアのリスト
RUST_LOG=info                       # ログレベル
DEBUG_MODE=true                     # デバッグモードの有効化
```

### 🧪 テスト結果と検証

Container Lab環境は以下を提供します：

✅ **ネットワーク基盤**
- 適切なノード間通信を持つ3ノードテストネット
- リアルデータチャネルを使用したWebRTC P2Pネットワーキング
- 環境ベースの設定システム

✅ **ブロックチェーン操作**
- ノード初期化とブロックチェーン起動
- トランザクション作成と伝播
- マルチノード協調

✅ **プロダクション対応**
- コンテナ化されたデプロイメント
- リソース監視とヘルスチェック
- 追加ノード用のスケーラブルアーキテクチャ

詳細なテスト手順と結果については、[E2Eテストレポート](e2e-test-report.md)を参照してください。

📚 ドキュメントリンク
導入ガイド (Getting Started)

Diamond IO コントラクト

開発者ガイド

APIリファレンス

この翻訳は日本語話者によって提供されています。内容に改善点があれば、お気軽にIssueやプルリクエストでご提案ください。

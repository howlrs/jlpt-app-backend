#!/bin/bash
set -euo pipefail

# JLPT Backend - Cloud Run デプロイスクリプト
# Usage: ./deploy.sh

PROJECT_ID="argon-depth-446413-t0"
SERVICE_NAME="backend"
REGION="asia-northeast1"

echo "=== JLPT Backend Deploy ==="
echo "Project: ${PROJECT_ID}"
echo "Service: ${SERVICE_NAME}"
echo "Region:  ${REGION}"
echo ""

# --- Pre-deploy checks ---

# 1. gcloud構成確認
ACTIVE_PROJECT=$(gcloud config get-value project 2>/dev/null)
if [ "$ACTIVE_PROJECT" != "$PROJECT_ID" ]; then
    echo "gcloud構成を jlpt に切り替えます..."
    gcloud config configurations activate jlpt
fi

# 2. Cargo ビルドチェック（コンパイルエラーを事前検知）
echo "--- Pre-deploy: Cargo check ---"
if ! cargo check 2>&1; then
    echo "ERROR: cargo check が失敗しました。デプロイを中止します"
    exit 1
fi
echo "--- Cargo check OK ---"
echo ""

# --- Deploy ---
echo "--- Cloud Run にデプロイ中... ---"
gcloud run deploy ${SERVICE_NAME} \
    --source . \
    --region=${REGION} \
    --allow-unauthenticated

# --- Post-deploy verification ---
echo ""
SERVICE_URL=$(gcloud run services describe ${SERVICE_NAME} --region=${REGION} --format="value(status.url)")
echo "=== デプロイ完了: ${SERVICE_URL} ==="
echo ""

echo "--- Post-deploy: ヘルスチェック ---"
sleep 5  # Cloud Run の起動待ち

# /api/meta エンドポイントの応答確認
API_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "${SERVICE_URL}/api/meta" --max-time 15 || echo "000")
if [ "${API_STATUS}" != "200" ]; then
    echo "WARNING: ${SERVICE_URL}/api/meta が HTTP ${API_STATUS} を返しました"
else
    echo "  /api/meta: OK (HTTP 200)"
fi

# /api/level/5/categories/1/questions エンドポイントのスモークテスト
QUESTIONS_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "${SERVICE_URL}/api/level/5/categories/1/questions?limit=1" --max-time 15 || echo "000")
if [ "${QUESTIONS_STATUS}" != "200" ]; then
    echo "WARNING: /api/level/5/categories/1/questions が HTTP ${QUESTIONS_STATUS} を返しました"
    echo "  問題取得APIに問題がある可能性があります"
else
    echo "  /api/questions: OK (HTTP 200)"
fi

echo ""
echo "=== 全チェック完了 ==="

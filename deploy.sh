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

# gcloud構成確認
ACTIVE_PROJECT=$(gcloud config get-value project 2>/dev/null)
if [ "$ACTIVE_PROJECT" != "$PROJECT_ID" ]; then
    echo "gcloud構成を jlpt に切り替えます..."
    gcloud config configurations activate jlpt
fi

# デプロイ
echo "Cloud Run にデプロイ中..."
gcloud run deploy ${SERVICE_NAME} \
    --source . \
    --region=${REGION} \
    --allow-unauthenticated

echo ""
echo "=== デプロイ完了 ==="
gcloud run services describe ${SERVICE_NAME} --region=${REGION} --format="value(status.url)"

#!/usr/bin/env bash
# Bootstrap AWS resources required to run the vector-test-harness.
# Run once per AWS account. Requires AWS credentials in the environment.
set -euo pipefail

REGION="${AWS_DEFAULT_REGION:-us-east-1}"

ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
echo "Account: ${ACCOUNT_ID}"
echo "Region:  ${REGION}"
echo ""

# S3 — Terraform state
STATE_BUCKET="vector-test-harness-state-${ACCOUNT_ID}"
if aws s3api head-bucket --bucket "$STATE_BUCKET" 2>/dev/null; then
  echo "✓ State bucket already exists: ${STATE_BUCKET}"
else
  aws s3api create-bucket --bucket "$STATE_BUCKET" --region "$REGION" > /dev/null
  echo "✓ Created state bucket: ${STATE_BUCKET}"
fi

# S3 — Test results
RESULTS_BUCKET="vector-test-harness-results-${ACCOUNT_ID}"
if aws s3api head-bucket --bucket "$RESULTS_BUCKET" 2>/dev/null; then
  echo "✓ Results bucket already exists: ${RESULTS_BUCKET}"
else
  aws s3api create-bucket --bucket "$RESULTS_BUCKET" --region "$REGION" > /dev/null
  echo "✓ Created results bucket: ${RESULTS_BUCKET}"
fi

# DynamoDB — Terraform state locking
if aws dynamodb describe-table --table-name TerraformLocks --region "$REGION" &>/dev/null; then
  echo "✓ DynamoDB table TerraformLocks already exists"
else
  aws dynamodb create-table \
    --table-name TerraformLocks \
    --attribute-definitions AttributeName=LockID,AttributeType=S \
    --key-schema AttributeName=LockID,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST \
    --region "$REGION" > /dev/null
  aws dynamodb wait table-exists --table-name TerraformLocks --region "$REGION"
  echo "✓ Created DynamoDB table: TerraformLocks"
fi

# IAM — instance profile for Packer builds (SSM access, no open SSH ports)
if aws iam get-instance-profile --instance-profile-name vector-packer-build &>/dev/null; then
  echo "✓ IAM instance profile vector-packer-build already exists"
else
  aws iam create-role \
    --role-name vector-packer-build \
    --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}' > /dev/null
  aws iam attach-role-policy \
    --role-name vector-packer-build \
    --policy-arn arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore
  aws iam create-instance-profile --instance-profile-name vector-packer-build > /dev/null
  aws iam add-role-to-instance-profile \
    --instance-profile-name vector-packer-build \
    --role-name vector-packer-build
  echo "✓ Created IAM instance profile: vector-packer-build"
fi

# SSH key
SSH_KEY="${HOME}/.ssh/vector_tests"
if [[ -f "$SSH_KEY" ]]; then
  echo "✓ SSH key already exists: ${SSH_KEY}"
else
  ssh-keygen -t ed25519 -f "$SSH_KEY" -N "" -C "vector-test-harness"
  echo "✓ Generated SSH key: ${SSH_KEY}"
fi

# Write .env
ENV_FILE="$(cd "$(dirname "$0")/.." && pwd)/.env"
cat > "$ENV_FILE" << EOF
export AWS_DEFAULT_REGION=${REGION}
export VECTOR_TEST_SSH_PRIVATE_KEY=~/.ssh/vector_tests
export VECTOR_TEST_SSH_PUBLIC_KEY=~/.ssh/vector_tests.pub
export VECTOR_TEST_USER_ID=$(whoami | tr -d '.')
export VECTOR_TEST_STATE_S3_BUCKET_NAME=${STATE_BUCKET}
export VECTOR_TEST_RESULTS_S3_BUCKET_NAME=${RESULTS_BUCKET}
EOF
echo "✓ Wrote ${ENV_FILE}"

echo ""
echo "Bootstrap complete. Next steps:"
echo "  1. source .env"
echo "  2. Build the AMI:  cd packer && packer build ami.json"
echo "  3. Run a test:     bin/test -t regex_parsing_performance -c sandbox -u \$(whoami)"

# Vector Test Harness — Setup & Benchmark Guide

End-to-end guide for running performance benchmarks from a fresh environment.

## Prerequisites

- macOS (ARM or x86) with Homebrew
- AWS account with EC2, S3, DynamoDB, IAM permissions
- AWS credentials configured (access key, role, or SSO)

---

## 1. Install dependencies

```bash
brew install gnu-getopt packer hashicorp/tap/terraform ansible
pip3 install --break-system-packages boto3 botocore awscli
```

Add gnu-getopt to your PATH (required for `bin/test`):

```bash
echo 'export PATH="$(brew --prefix)/opt/gnu-getopt/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

---

## 2. Clone the repo

```bash
git clone https://github.com/vectordotdev/vector-test-harness
cd vector-test-harness
```

---

## 3. Configure AWS credentials

Export standard AWS environment variables before running any command:

```bash
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SESSION_TOKEN=...      # if using temporary credentials
export AWS_DEFAULT_REGION=us-east-1
```

Or configure a profile in `~/.aws/credentials` and export:

```bash
export AWS_PROFILE=your-profile
export AWS_DEFAULT_REGION=us-east-1
```

---

## 4. Bootstrap AWS resources (one-time)

Run the setup script (see `scripts/bootstrap-aws.sh`), or manually:

```bash
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)

# S3 buckets
aws s3api create-bucket --bucket vector-test-harness-state-${ACCOUNT_ID} --region us-east-1
aws s3api create-bucket --bucket vector-test-harness-results-${ACCOUNT_ID} --region us-east-1

# DynamoDB for Terraform locking
aws dynamodb create-table \
  --table-name TerraformLocks \
  --attribute-definitions AttributeName=LockID,AttributeType=S \
  --key-schema AttributeName=LockID,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --region us-east-1

# IAM instance profile for Packer (lets build instances use SSM instead of open SSH)
aws iam create-role \
  --role-name vector-packer-build \
  --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}'
aws iam attach-role-policy \
  --role-name vector-packer-build \
  --policy-arn arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore
aws iam create-instance-profile --instance-profile-name vector-packer-build
aws iam add-role-to-instance-profile \
  --instance-profile-name vector-packer-build \
  --role-name vector-packer-build
```

---

## 5. Generate an SSH key

```bash
ssh-keygen -t ed25519 -f ~/.ssh/vector_tests -N "" -C "vector-test-harness"
```

---

## 6. Configure environment

Copy the example and fill in your values:

```bash
cp .env.example .env
```

Edit `.env`:

```bash
export AWS_DEFAULT_REGION=us-east-1
export VECTOR_TEST_SSH_PRIVATE_KEY=~/.ssh/vector_tests
export VECTOR_TEST_SSH_PUBLIC_KEY=~/.ssh/vector_tests.pub
export VECTOR_TEST_USER_ID=<your-username>
export VECTOR_TEST_STATE_S3_BUCKET_NAME=vector-test-harness-state-<account-id>
export VECTOR_TEST_RESULTS_S3_BUCKET_NAME=vector-test-harness-results-<account-id>
```

---

## 7. Build the AMI (one-time, ~20 min)

The AMI pre-installs all benchmark subjects (Fluent Bit, Fluentd, Logstash, etc.) so tests
start quickly without re-installing dependencies each run.

Install Packer plugins:

```bash
packer plugins install github.com/hashicorp/amazon
packer plugins install github.com/hashicorp/ansible
```

Build:

```bash
source .env

cd packer
export PREINSTALLED_TEST_SUBJECT_VERSIONS_ANSIBLE_VARS="\
preinstalled_filebeat_version=8.14.3 \
preinstalled_fluentbit_version=3.2.7 \
preinstalled_fluentd_version=4.5.2-1 \
preinstalled_logstash_version=8.17.3 \
preinstalled_splunk_heavy_forwarder_version=9.2.2-d76edf6f0a15 \
preinstalled_splunk_universal_forwarder_version=9.2.2-d76edf6f0a15 \
preinstalled_telegraf_version=1.33.2-1"

packer build ami.json
```

Note the AMI ID in the output — it's automatically used by Terraform.

---

## 8. Run a benchmark

```bash
source .env

bin/test \
  -t regex_parsing_performance \
  -c sandbox \
  -u <your-username> \
  -s vector
```

**Flags:**
- `-t` — test case name (see `cases/` directory)
- `-c` — configuration (e.g. `sandbox` for m5.large instances, `default` for original c5 fleet)
- `-u` — your user ID (namespaces all AWS resources)
- `-s` — subject to benchmark (vector, fluentbit, fluentd, logstash, filebeat, telegraf)
- `-v` — Vector version to test (default: `0.55.0`; use `nightly-YYYY-MM-DD` for nightlies)
- `--skip-terraform` — reuse existing instances (faster for repeated runs)
- `--run-only` — skip bootstrap, only run the test (instances must already be configured)
- `--bootstrap-only` — provision and configure instances but don't run tests yet

### Run all subjects sequentially

```bash
source .env

for SUBJECT in vector fluentbit fluentd logstash; do
  bin/test -t regex_parsing_performance -c sandbox -u <your-username> \
    -s $SUBJECT --skip-terraform $([ $SUBJECT != vector ] && echo "--run-only")
done
```

### Test a custom Vector build

```bash
source .env
export VECTOR_TEST_VECTOR_DEB_PATH=/path/to/vector_custom-1_amd64.deb

bin/test -t regex_parsing_performance -c sandbox -u <your-username> \
  -s vector -v dev-custom --skip-terraform
```

---

## 9. Read results

Consumer message throughput (for subjects with TCP output):

```bash
CONSUMER_IP=$(aws ec2 describe-instances \
  --region us-east-1 \
  --filters "Name=tag:TestUserID,Values=<your-username>" \
             "Name=tag:TestRole,Values=consumer" \
             "Name=instance-state-name,Values=running" \
  --query "Reservations[0].Instances[0].PublicIpAddress" \
  --output text)

ssh -i ~/.ssh/vector_tests ubuntu@$CONSUMER_IP \
  "sudo journalctl -u tcp_test_server --since '5 minutes ago' --no-pager | \
   grep 'messages' | grep -v ' 0 messages' | tail -15"
```

Compute events/sec from consecutive 5-second readings: `(count[n] - count[n-1]) / 5`.

Profiling data (CPU, memory, network) is uploaded to S3:

```bash
aws s3 ls s3://vector-test-harness-results-<account-id>/ --recursive
```

---

## 10. Teardown

Always tear down when done to avoid ongoing AWS costs:

```bash
source .env

cd cases/<test-name>/terraform
EGRESS_IP=$(curl -sf https://checkip.amazonaws.com)

terraform init \
  -backend-config="bucket=${VECTOR_TEST_STATE_S3_BUCKET_NAME}" \
  -backend-config="region=${AWS_DEFAULT_REGION}" \
  -backend-config="encrypt=true" \
  -backend-config="dynamodb_table=TerraformLocks" \
  -backend-config="key=vector-test-case/<test-name>/<config>/<user>.tfstate"

terraform destroy \
  -auto-approve \
  -var pub_key="${VECTOR_TEST_SSH_PUBLIC_KEY}" \
  -var test_name=<test-name> \
  -var test_configuration=<config> \
  -var user_id=<your-username> \
  -var results_s3_bucket_name=${VECTOR_TEST_RESULTS_S3_BUCKET_NAME} \
  -var ssh_cidr="${EGRESS_IP}/32" \
  -var-file="../configurations/<config>/terraform.tfvars"
```

---

## Sandbox configuration

The `sandbox` configuration (`cases/*/configurations/sandbox/`) uses affordable on-demand
m5.large instances (3 producers + 1 subject + 1 consumer) and is suitable for development
and iterative testing. The `default` configuration uses the original production-scale fleet.

To create a sandbox config for a test case:

```bash
mkdir -p cases/<test-name>/configurations/sandbox
cat > cases/<test-name>/configurations/sandbox/terraform.tfvars << 'EOF'
producer_instance_type  = "m5.large"
producer_instance_count = 3
subject_instance_type   = "m5.large"
subject_port            = 9000
consumer_instance_type  = "m5.large"
consumer_port           = 9000
EOF

cp cases/<test-name>/configurations/default/ansible.yml \
   cases/<test-name>/configurations/sandbox/ansible.yml
```

---

## Troubleshooting

**SSH times out mid-test** — your external IP changed. Update the security group:
```bash
NEW_IP=$(curl -sf https://checkip.amazonaws.com)
SG_ID=$(aws ec2 describe-security-groups \
  --filters "Name=group-name,Values=vector-test-<user>-<test>-default" \
  --query "SecurityGroups[0].GroupId" --output text)
aws ec2 revoke-security-group-ingress --group-id $SG_ID --protocol tcp --port 22 --cidr OLD_IP/32
aws ec2 authorize-security-group-ingress --group-id $SG_ID --protocol tcp --port 22 --cidr ${NEW_IP}/32
```

**Spot instances unavailable** — the repo now uses on-demand instances by default
(`aws_instance` in `terraform/aws_instance/main.tf`). If you need spot, revert to
`aws_spot_instance_request`.

**`include:` errors in Ansible** — all roles use `include_tasks:` (fixed for Ansible 2.16+).
If you see `ansible.builtin.include has been removed`, check for bare `include:` in task files.

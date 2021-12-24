provider "aws" {
  region  = "us-east-1"
  version = "~> 2.53"
}

terraform {
  required_version = ">= 0.12"
  backend "s3" {}
}

module "topology" {
  source = "../../../terraform/aws_uni_topology"

  providers = {
    aws = aws
  }

  pub_key                 = var.pub_key
  subject_instance_type   = var.subject_instance_type
  test_configuration      = var.test_configuration
  test_name               = var.test_name
  user_id                 = var.user_id
  results_s3_bucket_name  = var.results_s3_bucket_name
}

resource "aws_s3_bucket" "logs-bucket" {
  # data is namespaced by host within the bucket
  bucket = "file-to-s3-reliability-test-data"

  lifecycle_rule {
    enabled = true

    expiration {
      days = 14
    }
  }
}

data "aws_iam_policy_document" "logs-bucket-policy" {
  statement {
    sid = "AllowTestHarnessListBucket"

    actions = [
      "s3:ListBucket",
    ]

    resources = [
      aws_s3_bucket.logs-bucket.arn,
    ]
  }

  statement {
    sid = "AllowTestHarnessEverythingElse"

    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:DeleteObject",
    ]

    resources = [
      "${aws_s3_bucket.logs-bucket.arn}/*",
    ]
  }
}

resource "aws_iam_role_policy" "default" {
  role   = module.topology.instance_profile_name
  policy = data.aws_iam_policy_document.logs-bucket-policy.json
}

provider "aws" {
  region  = "us-east-1"
  version = "~> 2.53"
}

terraform {
  required_version = ">= 0.12"
  backend "s3" {}
}

module "topology" {
  source = "../../../terraform/aws_tcp_bi_receive_topology"

  providers = {
    aws = aws
  }

  producer_instance_count = var.producer_instance_count
  producer_instance_type  = var.producer_instance_type
  pub_key                 = var.pub_key
  subject_instance_type   = var.subject_instance_type
  subject_port            = var.subject_port
  test_configuration      = var.test_configuration
  test_name               = var.test_name
  user_id                 = var.user_id
  results_s3_bucket_name  = var.results_s3_bucket_name
}

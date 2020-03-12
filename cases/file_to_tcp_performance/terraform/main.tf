provider "aws" {
  region  = "us-east-1"
  version = "~> 2.12"
}

terraform {
  required_version = ">= 0.12"
  backend "s3" {}
}

module "topology" {
  source = "../../../terraform/aws_tcp_bi_topology"

  providers = {
    aws = aws
  }

  consumer_instance_type  = var.consumer_instance_type
  consumer_port           = var.consumer_port
  pub_key                 = var.pub_key
  subject_instance_type   = var.subject_instance_type
  test_configuration      = var.test_configuration
  test_name               = var.test_name
  user_id                 = var.user_id
  results_s3_bucket_name  = var.results_s3_bucket_name
}

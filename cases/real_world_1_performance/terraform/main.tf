provider "aws" {
  version = "~> 3.11"
}

terraform {
  required_version = ">= 0.13"
  backend "s3" {}
}

module "topology" {
  source = "../../../terraform/aws_tcp_tri_topology"

  providers = {
    aws = aws
  }

  consumer_instance_type  = var.consumer_instance_type
  consumer_port           = var.consumer_port
  producer_instance_count = var.producer_instance_count
  producer_instance_type  = var.producer_instance_type
  pub_key                 = var.pub_key
  subject_instance_type   = var.subject_instance_type
  subject_from_port       = var.subject_api_port
  subject_to_port         = var.subject_audit_port
  test_configuration      = var.test_configuration
  test_name               = var.test_name
  user_id                 = var.user_id
  results_s3_bucket_name  = var.results_s3_bucket_name
}

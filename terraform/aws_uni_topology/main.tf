data "aws_caller_identity" "current" {
}

locals {
  availability_zone      = "us-east-1b"
  cidr_block             = "10.0.0.0/16"
  test_configuration     = var.test_configuration
  test_name              = var.test_name
  user_id                = var.user_id
  results_s3_bucket_name = var.results_s3_bucket_name
}

module "vpc" {
  source = "../../../terraform/aws_vpc"

  providers = {
    aws = aws
  }

  availability_zone = local.availability_zone
  cidr_block        = local.cidr_block
  test_name         = local.test_name
  user_id           = local.user_id
}

module "aws_instance_profile" {
  source = "../../../terraform/aws_instance_profile"

  providers = {
    aws = aws
  }

  test_name              = local.test_name
  user_id                = local.user_id
  results_s3_bucket_name = local.results_s3_bucket_name
}

module "aws_instance_subject" {
  source = "../../../terraform/aws_instance"

  providers = {
    aws = aws
  }

  availability_zone     = local.availability_zone
  instance_profile_name = module.aws_instance_profile.name
  instance_type         = var.subject_instance_type
  public_key            = file(var.pub_key)
  role_name             = "subject"
  security_group_ids    = [module.vpc.default_security_group_id]
  subnet_id             = module.vpc.default_subnet_id
  test_configuration    = local.test_configuration
  test_name             = local.test_name
  user_id               = local.user_id
}

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
  source = "../aws_vpc"

  providers = {
    aws = aws
  }

  availability_zone = local.availability_zone
  cidr_block        = local.cidr_block
  test_name         = local.test_name
  user_id           = local.user_id
}

resource "aws_security_group" "subject" {
  name        = "vector-test-${local.user_id}-${local.test_name}-subject"
  description = "Security group for members of the Vector ${local.user_id}-${local.test_name} test subject group"
  vpc_id      = module.vpc.vpc_id

  # Inter-node egress
  egress {
    from_port = var.consumer_port
    to_port   = var.consumer_port
    protocol  = "tcp"
    self      = true
  }

  tags = {
    Name       = "vector-test-${local.user_id}-${local.test_name}-subject"
    TestName   = local.test_name
    TestRole   = "subject"
    TestUserID = local.user_id
  }
}

resource "aws_security_group" "consumer" {
  name        = "vector-test-${local.user_id}-${local.test_name}-consumer"
  description = "Security group for members of the Vector ${local.user_id}-${local.test_name} test consumer group"
  vpc_id      = module.vpc.vpc_id

  # Inter-node ingress
  ingress {
    from_port = var.consumer_port
    to_port   = var.consumer_port
    protocol  = "tcp"
    # TF-UPGRADE-TODO: In Terraform v0.10 and earlier, it was sometimes necessary to
    # force an interpolation expression to be interpreted as a list by wrapping it
    # in an extra set of list brackets. That form was supported for compatibilty in
    # v0.11, but is no longer supported in Terraform v0.12.
    #
    # If the expression in the following list itself returns a list, remove the
    # brackets to avoid interpretation as a list of lists. If the expression
    # returns a single list item then leave it as-is and remove this TODO comment.
    cidr_blocks = [module.vpc.cidr_block]
  }

  tags = {
    Name       = "vector-test-${local.user_id}-${local.test_name}-consumer"
    TestName   = local.test_name
    TestRole   = "consumer"
    TestUserID = local.user_id
  }
}

module "aws_instance_profile" {
  source = "../aws_instance_profile"

  providers = {
    aws = aws
  }

  test_configuration     = local.test_configuration
  test_name              = local.test_name
  user_id                = local.user_id
  results_s3_bucket_name = local.results_s3_bucket_name
}

module "aws_instance_subject" {
  source = "../aws_instance"

  providers = {
    aws = aws
  }

  availability_zone     = local.availability_zone
  instance_profile_name = module.aws_instance_profile.name
  instance_type         = var.subject_instance_type
  public_key            = file(var.pub_key)
  role_name             = "subject"
  security_group_ids    = [module.vpc.default_security_group_id, aws_security_group.subject.id]
  subnet_id             = module.vpc.default_subnet_id
  test_configuration    = local.test_configuration
  test_name             = local.test_name
  user_id               = local.user_id
}

module "aws_instance_consumer" {
  source = "../aws_instance"

  providers = {
    aws = aws
  }

  availability_zone     = local.availability_zone
  instance_profile_name = module.aws_instance_profile.name
  instance_type         = var.consumer_instance_type
  public_key            = file(var.pub_key)
  role_name             = "consumer"
  security_group_ids    = [module.vpc.default_security_group_id, aws_security_group.consumer.id]
  subnet_id             = module.vpc.default_subnet_id
  test_configuration    = local.test_configuration
  test_name             = local.test_name
  user_id               = local.user_id
}

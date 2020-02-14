provider "aws" {}

data "aws_region" "current" {}

data "aws_caller_identity" "current" {}

data "aws_ami" "ami" {
  most_recent = true

  filter {
    name   = "name"
    values = ["vector/images/hvm-ssd/test-harness-amd64-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }

  owners = [data.aws_caller_identity.current.account_id]
}

resource "aws_spot_instance_request" "default" {
  count = var.instance_count

  ami                         = data.aws_ami.ami.id
  associate_public_ip_address = true
  availability_zone           = var.availability_zone
  iam_instance_profile        = var.instance_profile_name
  instance_type               = var.instance_type
  monitoring                  = false
  spot_type                   = "one-time"
  subnet_id                   = var.subnet_id
  vpc_security_group_ids      = var.security_group_ids
  wait_for_fulfillment        = true

  root_block_device {
    volume_type           = "gp2"
    volume_size           = "30"
    delete_on_termination = true
  }

  user_data = <<-EOT
    #cloud-config
    ssh_authorized_keys:
    - ${var.public_key}
  EOT

  tags = {
    Name              = "vector-test-${var.user_id}-${var.test_name}-${var.test_configuration}-${var.role_name}"
    Test              = "true"
    TestName          = var.test_name
    TestConfiguration = var.test_configuration
    TestRole          = var.role_name
    TestUserID        = var.user_id
  }

  # See https://github.com/terraform-providers/terraform-provider-aws/issues/174
  provisioner "local-exec" {
    command = "aws ec2 create-tags --resources ${self.spot_instance_id} --tags Key=Name,Value=${self.tags.Name} Key=Test,Value=${self.tags.Test} Key=TestName,Value=${self.tags.TestName} Key=TestConfiguration,Value=${self.tags.TestConfiguration} Key=TestRole,Value=${self.tags.TestRole} Key=TestUserID,Value=${self.tags.TestUserID} --region ${data.aws_region.current.name}"
  }
}

resource "aws_cloudwatch_metric_alarm" "terminate" {
  count = var.instance_count

  alarm_name          = "vector-test-${var.user_id}-${var.test_name}-${var.role_name}-${count.index}"
  namespace           = "AWS/EC2"
  evaluation_periods  = "45"
  period              = "120"
  alarm_description   = "This metric terminates stale instances"
  alarm_actions       = ["arn:aws:automate:${data.aws_region.current.name}:ec2:terminate"]
  statistic           = "Maximum"
  comparison_operator = "LessThanThreshold"
  threshold           = "5"
  metric_name         = "CPUUtilization"

  dimensions = {
    InstanceId = aws_spot_instance_request.default[count.index].spot_instance_id
  }
}

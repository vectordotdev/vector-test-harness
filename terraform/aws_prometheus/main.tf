provider "aws" {}

module "aws_instance" {
  source = "../aws_instance"

  providers = {
    "aws" = "aws"
  }

  availability_zone  = "${var.availability_zone}"
  key_name           = "${var.key_name}"
  role_name          = "prometheus"
  security_group_ids = ["${var.security_group_ids}"]
  subnet_id          = "${var.subnet_id}"
  test_name          = "${var.test_name}"
  user_id            = "${var.user_id}"
}

resource "aws_iam_role" "default" {
  name               = "vector-test-${var.user_id}-${var.test_name}-prometheus"
  path               = "/test/${var.user_id}/${var.test_name}/"
  assume_role_policy = "${data.aws_iam_policy_document.role_sts.json}"
}

data "aws_iam_policy_document" "role_sts" {
  statement {
    sid = "AllowEC2Assume"

    principals = {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }

    actions = [
      "sts:AssumeRole",
    ]
  }
}

resource "aws_iam_policy" "default" {
  name        = "Test${title(var.user_id)}${title(var.test_name)}"
  path        = "/test/${var.user_id}/${var.test_name}/"
  description = "Allows Prometheus to gather information about EC2 instances"
  policy      = "${data.aws_iam_policy_document.default.json}"
}

data "aws_iam_policy_document" "default" {
  statement {
    sid = "AllowEC2Descriptions"

    actions = [
      "ec2:DescribeInstances",
    ]

    resources = [
      "*",
    ]
  }
}

resource "aws_iam_role_policy_attachment" "default" {
  role       = "${aws_iam_role.default.name}"
  policy_arn = "${aws_iam_policy.default.arn}"
}

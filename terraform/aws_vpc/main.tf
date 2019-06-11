provider "aws" {}

resource "aws_vpc" "default" {
  cidr_block           = "${var.cidr_block}"
  instance_tenancy     = "default"
  enable_dns_support   = true
  enable_dns_hostnames = true
  enable_classiclink   = false

  tags = {
    Name       = "vector-test-${var.user_id}-${var.test_name}"
    TestName   = "${var.test_name}"
    TestUserID = "${var.user_id}"
  }
}

resource "aws_internet_gateway" "default" {
  vpc_id = "${aws_vpc.default.id}"

  tags = {
    Name       = "vector-test-${var.user_id}-${var.test_name}"
    TestName   = "${var.test_name}"
    TestUserID = "${var.user_id}"
  }
}

resource "aws_route_table" "default" {
  vpc_id = "${aws_vpc.default.id}"

  tags = {
    Name       = "vector-test-${var.user_id}-${var.test_name}"
    TestName   = "${var.test_name}"
    TestUserID = "${var.user_id}"
  }
}

resource "aws_route_table_association" "a" {
  subnet_id      = "${aws_subnet.default.id}"
  route_table_id = "${aws_route_table.default.id}"
}

resource "aws_route" "internet_gateway" {
  route_table_id         = "${aws_route_table.default.id}"
  destination_cidr_block = "0.0.0.0/0"
  gateway_id             = "${aws_internet_gateway.default.id}"
}

resource "aws_security_group" "default" {
  name        = "vector-test-${var.user_id}-${var.test_name}-default"
  description = "Security group for members of the Vector ${var.user_id}-${var.test_name} test"
  vpc_id      = "${aws_vpc.default.id}"

  # SSH access
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Outbound to anywhere
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_subnet" "default" {
  vpc_id                  = "${aws_vpc.default.id}"
  availability_zone       = "${var.availability_zone}"
  cidr_block              = "${aws_vpc.default.cidr_block}"
  map_public_ip_on_launch = false

  tags = {
    Name       = "vector-test-${var.test_name}"
    TestName   = "${var.test_name}"
    TestUserID = "${var.user_id}"
  }
}
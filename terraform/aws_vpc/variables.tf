variable "availability_zone" {
  type = string

  description = <<EOF
The AWS availability zone.
EOF
}

variable "cidr_block" {
  type = string

  description = <<EOF
The VPC CIDR block.
EOF
}

variable "test_name" {
  type = string

  description = <<EOF
The name of the current test.
EOF
}

variable "user_id" {
  type = string

  description = <<EOF
The current user ID.
EOF
}

variable "ssh_cidr" {
  type    = string
  default = "0.0.0.0/0"

  description = <<EOF
CIDR block allowed SSH access to test instances. Override with your
egress IP (e.g. "1.2.3.4/32") to satisfy security policies that
prohibit broadly permissive inbound rules.
EOF
}

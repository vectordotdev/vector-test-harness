variable "availability_zone" {
  type = "string"

  description = <<EOF
The AWS availability zone.
EOF
}

variable "cidr_block" {
  type = "string"

  description = <<EOF
The VPC CIDR block.
EOF
}

variable "test_name" {
  type = "string"

  description = <<EOF
The name of the current test.
EOF
}

variable "user_id" {
  type = "string"

  description = <<EOF
The current user ID.
EOF
}
variable "availability_zone" {
  type = "string"

  description = <<EOF
The AWS availability zone.
EOF
}

variable "key_name" {
  type = "string"
  default = "t3.micro"

  description = <<EOF
Name of the key pair to use.
EOF
}

variable "security_group_ids" {
  type = "list"

  description = <<EOF
A list of EC2 security group IDs to apply to the EC2 instance(s) started
EOF
}

variable "subnet_id" {
  type = "string"

  description = <<EOF
The subnet ID to place the instance in.
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
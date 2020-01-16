variable "availability_zone" {
  type = "string"

  description = <<EOF
The AWS availability zone.
EOF
}

variable "instance_count" {
  type = "string"
  default = 1

  description = <<EOF
The number of instances to launch.
EOF
}

variable "instance_profile_name" {
  type = "string"

  description = <<EOF
The IAM instance profile name.
EOF
}

variable "instance_type" {
  type = "string"
  default = "t3.micro"

  description = <<EOF
The type / class of instance.
EOF
}

variable "key_name" {
  type = "string"

  description = <<EOF
Name of the key pair to use.
EOF
}

variable "role_name" {
  type = "string"

  description = <<EOF
The name of the instance role.
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

variable "test_configuration" {
  type = "string"

  description = <<EOF
The configuration name of the current test.
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

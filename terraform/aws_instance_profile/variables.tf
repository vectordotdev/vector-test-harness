variable "test_configuration" {
  type = string

  description = <<EOF
The configuration name of the current test.
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

variable "results_s3_bucket_name" {
  type = string

  description = <<EOF
The name of the S3 bucket to upload the results to.
EOF
}

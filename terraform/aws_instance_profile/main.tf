data "aws_arn" "results_s3_bucket" {
  arn = "arn:aws:s3:::${var.results_s3_bucket_name}"
}

resource "aws_iam_instance_profile" "default" {
  name = "vector-test-${var.user_id}-${var.test_name}"
  path = "/vector-test/${var.user_id}/${var.test_name}/"
  role = aws_iam_role.default.name
}

resource "aws_iam_role" "default" {
  name               = "vector-test-${var.user_id}-${var.test_name}"
  path               = "/vector-test/${var.user_id}/${var.test_name}/"
  assume_role_policy = data.aws_iam_policy_document.allow_ec2_assume.json
}

resource "aws_iam_role_policy_attachment" "default" {
  role       = aws_iam_role.default.name
  policy_arn = aws_iam_policy.default.arn
}

data "aws_iam_policy_document" "allow_ec2_assume" {
  statement {
    sid = "AllowEC2Assume"

    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }

    actions = [
      "sts:AssumeRole",
    ]
  }
}

resource "aws_iam_policy" "default" {
  name        = "VectorTest${title(var.user_id)}${title(var.test_name)}InstanceProfile"
  path        = "/vector-test/${var.user_id}/${var.test_name}/"
  description = "Instance profile policy for the ${var.user_id}/${var.test_name} test"
  policy      = data.aws_iam_policy_document.default.json
}

data "aws_iam_policy_document" "default" {
  statement {
    sid = "TestResultUploadAccess"

    actions = [
      "s3:PutObject",
    ]

    resources = [
      "${data.aws_arn.results_s3_bucket.arn}/name=${var.test_name}/*"
    ]
  }
}

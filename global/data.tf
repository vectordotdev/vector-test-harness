resource "aws_s3_bucket" "vector-tests" {
  bucket = var.results-s3-bucket-name
  acl    = "public-read"

  tags = {
    name = "Vector Tests"
  }

  versioning {
    enabled = true
  }

  cors_rule {
    allowed_headers = ["*"]
    allowed_methods = ["GET"]
    allowed_origins = ["*"]
    max_age_seconds = 3000
  }

  website {
    index_document = "index.html"
    error_document = "index.html"
  }

  lifecycle_rule {
    id      = "delete_old_versions"
    prefix  = ""
    enabled = true

    noncurrent_version_expiration {
      days = 14
    }
  }
  lifecycle_rule {
    id      = "abort_incomplete_multipart_uploads"
    prefix  = ""
    enabled = true

    abort_incomplete_multipart_upload_days = 1
  }
  lifecycle_rule {
    id      = "delete_expired_delete_markers"
    prefix  = ""
    enabled = true

    expiration {
      expired_object_delete_marker = true
    }
  }

  lifecycle {
    prevent_destroy = "true"
  }
}

data "aws_iam_policy_document" "vector-tests" {
  statement {
    sid = "AllowEveryoneReadObjects"

    actions = [
      "s3:GetObject",
    ]

    principals {
      type        = "AWS"
      identifiers = ["*"]
    }

    resources = [
      "${aws_s3_bucket.vector-tests.arn}/*",
    ]
  }

  statement {
    sid = "AllowEveryoneListBucket"

    actions = [
      "s3:ListBucket",
    ]

    principals {
      type        = "AWS"
      identifiers = ["*"]
    }

    resources = [
      aws_s3_bucket.vector-tests.arn,
    ]
  }
}

resource "aws_s3_bucket_policy" "vector-tests" {
  bucket = aws_s3_bucket.vector-tests.id
  policy = data.aws_iam_policy_document.vector-tests.json
}

resource "aws_s3_bucket_object" "vector-tests-index" {
  bucket       = aws_s3_bucket.vector-tests.id
  key          = "index.html"
  source       = "data/index.html"
  content_type = "text/html"
}

resource "aws_s3_bucket" "vector-test-athena-results" {
  bucket = "vector-test-athena-results"
  acl    = "private"

  tags = {
    name = "Vector Tests Athena Results"
  }

  lifecycle_rule {
    id      = "delete_all"
    prefix  = ""
    enabled = true

    expiration {
      days = 30
    }
  }

  # lifecycle {
  #   prevent_destroy = "true"
  # }
}

resource "aws_glue_catalog_database" "vector_tests" {
  name = "vector_tests"
}

resource "aws_glue_catalog_table" "vector_tests" {
  name          = "vector_tests"
  database_name = "vector_tests"

  table_type = "EXTERNAL_TABLE"

  parameters = {
    EXTERNAL = "TRUE"
  }

  partition_keys {
    name = "name"
    type = "string"
  }
  partition_keys {
    name = "configuration"
    type = "string"
  }
  partition_keys {
    name = "subject"
    type = "string"
  }
  partition_keys {
    name = "version"
    type = "string"
  }
  partition_keys {
    name = "timestamp"
    type = "int"
  }
  partition_keys {
    name = "hostname"
    type = "string"
  }

  storage_descriptor {
    location      = "s3://${aws_s3_bucket.vector-tests.bucket}/"
    input_format  = "org.apache.hadoop.mapred.TextInputFormat"
    output_format = "org.apache.hadoop.hive.ql.io.HiveIgnoreKeyTextOutputFormat"

    ser_de_info {
      name                  = "vector-tests"
      serialization_library = "org.apache.hadoop.hive.serde2.OpenCSVSerde"

      parameters = {
        "skip.header.line.count" = 7
      }
    }

    columns {
      name = "epoch"
      type = "double"
    }
    columns {
      name = "cpu_usr"
      type = "double"
    }
    columns {
      name = "cpu_sys"
      type = "double"
    }
    columns {
      name = "cpu_idl"
      type = "double"
    }
    columns {
      name = "cpu_wai"
      type = "double"
    }
    columns {
      name = "cpu_hiq"
      type = "double"
    }
    columns {
      name = "cpu_siq"
      type = "double"
    }
    columns {
      name = "disk_read"
      type = "double"
    }
    columns {
      name = "disk_writ"
      type = "double"
    }
    columns {
      name = "io_read"
      type = "double"
    }
    columns {
      name = "io_writ"
      type = "double"
    }
    columns {
      name = "load_avg_1m"
      type = "double"
    }
    columns {
      name = "load_avg_5m"
      type = "double"
    }
    columns {
      name = "load_avg_15m"
      type = "double"
    }
    columns {
      name = "mem_used"
      type = "double"
    }
    columns {
      name = "mem_buff"
      type = "double"
    }
    columns {
      name = "mem_cach"
      type = "double"
    }
    columns {
      name = "mem_free"
      type = "double"
    }
    columns {
      name = "net_recv"
      type = "double"
    }
    columns {
      name = "net_send"
      type = "double"
    }
    columns {
      name = "procs_run"
      type = "double"
    }
    columns {
      name = "procs_bulk"
      type = "double"
    }
    columns {
      name = "procs_new"
      type = "double"
    }
    columns {
      name = "procs_total"
      type = "double"
    }
    columns {
      name = "sys_init"
      type = "double"
    }
    columns {
      name = "sys_csw"
      type = "double"
    }
    columns {
      name = "sock_total"
      type = "double"
    }
    columns {
      name = "sock_tcp"
      type = "double"
    }
    columns {
      name = "sock_udp"
      type = "double"
    }
    columns {
      name = "sock_raw"
      type = "double"
    }
    columns {
      name = "sock_frg"
      type = "double"
    }
    columns {
      name = "tcp_lis"
      type = "double"
    }
    columns {
      name = "tcp_act"
      type = "double"
    }
    columns {
      name = "tcp_syn"
      type = "double"
    }
    columns {
      name = "tcp_tim"
      type = "double"
    }
    columns {
      name = "tcp_clo"
      type = "double"
    }
  }
}


resource "aws_s3_bucket" "index" {
  bucket = "openlore-indexer-index-091153021562"

  lifecycle {
    prevent_destroy = true
  }
}

resource "aws_s3_bucket_versioning" "index" {
  bucket = aws_s3_bucket.index.id

  versioning_configuration {
    status = "Enabled"
  }
}

# A week of history, and never fewer than the 5 newest versions, whatever their age.
resource "aws_s3_bucket_lifecycle_configuration" "index" {
  bucket = aws_s3_bucket.index.id

  rule {
    id     = "index-history"
    status = "Enabled"

    filter {}

    noncurrent_version_expiration {
      noncurrent_days           = 7
      newer_noncurrent_versions = 5
    }

    expiration {
      expired_object_delete_marker = true
    }

    abort_incomplete_multipart_upload {
      days_after_initiation = 1
    }
  }

  depends_on = [aws_s3_bucket_versioning.index]
}

resource "aws_s3_bucket_server_side_encryption_configuration" "index" {
  bucket = aws_s3_bucket.index.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "index" {
  bucket                  = aws_s3_bucket.index.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "index" {
  bucket = aws_s3_bucket.index.id

  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

data "aws_iam_policy_document" "index_is_tls_only" {
  statement {
    sid       = "DenyPlainTransport"
    effect    = "Deny"
    actions   = ["s3:*"]
    resources = [aws_s3_bucket.index.arn, "${aws_s3_bucket.index.arn}/*"]

    principals {
      type        = "*"
      identifiers = ["*"]
    }

    condition {
      test     = "Bool"
      variable = "aws:SecureTransport"
      values   = ["false"]
    }
  }
}

resource "aws_s3_bucket_policy" "index" {
  bucket = aws_s3_bucket.index.id
  policy = data.aws_iam_policy_document.index_is_tls_only.json

  depends_on = [aws_s3_bucket_public_access_block.index]
}

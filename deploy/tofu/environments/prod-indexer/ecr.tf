resource "aws_ecr_repository" "indexer" {
  name                 = "openlore-indexer"
  image_tag_mutability = "IMMUTABLE"
  force_delete         = false

  encryption_configuration {
    encryption_type = "AES256"
  }

  image_scanning_configuration {
    scan_on_push = true
  }
}

# Ten leaves room over the 5 versions the prune keeps. The lifecycle counts pushes and knows
# nothing about aliases, so deploy.sh refuses a copy that would evict a digest still in use
# (U-STZ-9).
resource "aws_ecr_lifecycle_policy" "indexer" {
  repository = aws_ecr_repository.indexer.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Untagged images expire after a day."
        selection = {
          tagStatus   = "untagged"
          countType   = "sinceImagePushed"
          countUnit   = "days"
          countNumber = 1
        }
        action = { type = "expire" }
      },
      {
        rulePriority = 2
        description  = "Keep the newest 10 builds."
        selection = {
          tagStatus     = "tagged"
          tagPrefixList = ["sha-"]
          countType     = "imageCountMoreThan"
          countNumber   = 10
        }
        action = { type = "expire" }
      },
    ]
  })
}

data "aws_iam_policy_document" "lambda_pulls_images" {
  statement {
    sid     = "LambdaPullsImages"
    actions = ["ecr:BatchGetImage", "ecr:GetDownloadUrlForLayer"]

    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }

    condition {
      test     = "StringLike"
      variable = "aws:SourceArn"
      values   = ["arn:aws:lambda:${local.region}:${local.account_id}:function:openlore-indexer-*"]
    }
  }
}

# Tofu owns this policy so Lambda does not write its own and drift it.
resource "aws_ecr_repository_policy" "indexer" {
  repository = aws_ecr_repository.indexer.name
  policy     = data.aws_iam_policy_document.lambda_pulls_images.json
}

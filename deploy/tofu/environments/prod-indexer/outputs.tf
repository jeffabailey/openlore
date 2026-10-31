output "index_bucket" { value = aws_s3_bucket.index.bucket }
output "ecr_repository_url" { value = aws_ecr_repository.indexer.repository_url }
output "lambda_log_group" { value = local.lambda_log_group }
output "search_live_url" { value = aws_lambda_function_url.search_live.function_url }
output "search_candidate_url" { value = aws_lambda_function_url.search_candidate.function_url }
output "distribution_domain" { value = aws_cloudfront_distribution.index.domain_name }

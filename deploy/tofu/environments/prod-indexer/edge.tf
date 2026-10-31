# The distribution the probe's address comes from, and only that. The edge function, the strip
# policy, the certificate and the other two behaviors are the next step's, and this root is not
# applied before they land.
resource "aws_cloudfront_distribution" "index" {
  enabled = true

  origin {
    origin_id   = "search-live"
    domain_name = trimsuffix(trimprefix(aws_lambda_function_url.search_live.function_url, "https://"), "/")

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }

  default_cache_behavior {
    target_origin_id       = "search-live"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD"]
    cache_policy_id        = "4135ea2d-6df8-44a3-9df3-4b5a84be39ad"
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    cloudfront_default_certificate = true
  }

  # lambda stop owns this value, so an apply never undoes a stop (D-STZ-18d).
  lifecycle {
    ignore_changes = [enabled]
  }
}

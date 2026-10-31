# The public edge: one allowlist in front of every path (D-STZ-21). Each behavior is its own block
# so each one can be read, and each allows every method: the function answers what is not allowed,
# with a 404.

locals {
  index_domain = "index.openlore.jeffbailey.us"
}

# Validation waits on a CNAME the operator adds in Cloudflare by hand (D-STZ-34), and the
# distribution takes the certificate through it, so the bootstrap's first apply stops at the
# certificate.
resource "aws_acm_certificate" "index" {
  domain_name       = local.index_domain
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_acm_certificate_validation" "index" {
  certificate_arn = aws_acm_certificate.index.arn
}

# The record the operator adds in Cloudflare.
output "index_certificate_validation" {
  value = aws_acm_certificate.index.domain_validation_options
}

resource "aws_cloudfront_function" "edge" {
  name    = "openlore-index-edge"
  runtime = "cloudfront-js-2.0"
  comment = "Search and the health check; 404 to everything else."
  publish = true
  code    = file("${path.module}/edge-function.js")
}

# CloudFront has no wildcard here, so deploy.sh's edge check asserts no x-amzn- header survives.
resource "aws_cloudfront_response_headers_policy" "strip" {
  name = "openlore-index-strip"

  remove_headers_config {
    items {
      header = "x-amzn-requestid"
    }
    items {
      header = "x-amzn-trace-id"
    }
    items {
      header = "x-amzn-remapped-content-length"
    }
    items {
      header = "x-amzn-remapped-connection"
    }
    items {
      header = "x-amzn-remapped-date"
    }
    items {
      header = "x-amzn-remapped-x-amzn-requestid"
    }
  }
}

data "aws_cloudfront_cache_policy" "caching_disabled" {
  name = "Managed-CachingDisabled"
}

data "aws_cloudfront_origin_request_policy" "all_viewer_except_host" {
  name = "Managed-AllViewerExceptHostHeader"
}

resource "aws_cloudfront_distribution" "index" {
  enabled         = true
  aliases         = [local.index_domain]
  price_class     = "PriceClass_100"
  http_version    = "http2and3"
  is_ipv6_enabled = true

  origin {
    origin_id   = "search-live"
    domain_name = trimsuffix(trimprefix(aws_lambda_function_url.search_live.function_url, "https://"), "/")

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
      origin_read_timeout    = 10
    }
  }

  ordered_cache_behavior {
    path_pattern               = "/xrpc/org.openlore.appview.searchClaims"
    target_origin_id           = "search-live"
    viewer_protocol_policy     = "redirect-to-https"
    allowed_methods            = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods             = ["GET", "HEAD"]
    cache_policy_id            = data.aws_cloudfront_cache_policy.caching_disabled.id
    origin_request_policy_id   = data.aws_cloudfront_origin_request_policy.all_viewer_except_host.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.strip.id

    function_association {
      event_type   = "viewer-request"
      function_arn = aws_cloudfront_function.edge.arn
    }
  }

  ordered_cache_behavior {
    path_pattern               = "/healthz"
    target_origin_id           = "search-live"
    viewer_protocol_policy     = "redirect-to-https"
    allowed_methods            = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods             = ["GET", "HEAD"]
    cache_policy_id            = data.aws_cloudfront_cache_policy.caching_disabled.id
    origin_request_policy_id   = data.aws_cloudfront_origin_request_policy.all_viewer_except_host.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.strip.id

    function_association {
      event_type   = "viewer-request"
      function_arn = aws_cloudfront_function.edge.arn
    }
  }

  # Everything else: the function always answers this one with a 404.
  default_cache_behavior {
    target_origin_id           = "search-live"
    viewer_protocol_policy     = "redirect-to-https"
    allowed_methods            = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods             = ["GET", "HEAD"]
    cache_policy_id            = data.aws_cloudfront_cache_policy.caching_disabled.id
    origin_request_policy_id   = data.aws_cloudfront_origin_request_policy.all_viewer_except_host.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.strip.id

    function_association {
      event_type   = "viewer-request"
      function_arn = aws_cloudfront_function.edge.arn
    }
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.index.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  # lambda stop owns this value, so an apply never undoes a stop (D-STZ-18d).
  lifecycle {
    ignore_changes = [enabled]
  }
}

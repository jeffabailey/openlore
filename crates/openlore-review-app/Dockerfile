# openlore-review-app runtime image (ADR-075, devops/ci-cd-pipeline.md §3.2).
#
# RUNTIME ONLY: no build stage, no compiler. CI (ci.yml `review-app-build`) compiles the arm64
# binary inside a rust:1-bookworm job container so its glibc matches debian12 (2.36), then
# builds this file with a context holding just that binary:
#
#   docker buildx build --platform linux/arm64 -f crates/openlore-review-app/Dockerfile <dir-with-binary>
#
# The base is pinned by index digest; bump it deliberately (trivy gates every build).
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f

LABEL org.opencontainers.image.source="https://github.com/jeffabailey/openlore" \
      org.opencontainers.image.title="openlore-review-app" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --chmod=0555 openlore-review-app /openlore-review-app

# distroless nonroot = 65532:65532; the host's /pds/app/{data,secrets} are owned by it.
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/openlore-review-app"]
CMD ["serve"]

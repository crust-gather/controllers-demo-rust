FROM cgr.dev/chainguard/static
ADD --chown=nonroot:nonroot _out/controller /app/
EXPOSE 8080
ENTRYPOINT ["/app/controller"]

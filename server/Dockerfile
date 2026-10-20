# Native executable is compiled/tested by CI before packaging, just like WASM.
# Build on glibc <= 2.41 (CI Ubuntu); the runtime supplies glibc and libgcc.
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*
COPY --chmod=0555 dist/server/ryggattack-server /usr/local/bin/ryggattack-server
USER 10001:10001
EXPOSE 8787 8788
ENTRYPOINT ["/usr/local/bin/ryggattack-server"]
CMD ["0.0.0.0:8787"]

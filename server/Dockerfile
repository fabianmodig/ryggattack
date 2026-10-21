# Native executable is compiled/tested by CI before packaging, just like WASM.
# Build on glibc <= 2.41 (CI Ubuntu); the runtime supplies glibc and libgcc.
FROM debian:trixie-slim
# The base already supplies libgcc-s1 (also required by apt itself). Assert it
# instead of running a redundant network install that needs an _apt UID mapping.
RUN ldconfig -p | grep -q 'libgcc_s.so.1'
COPY --chmod=0555 dist/server/ryggattack-server /usr/local/bin/ryggattack-server
USER 10001:10001
EXPOSE 8787 8788
ENTRYPOINT ["/usr/local/bin/ryggattack-server"]
CMD ["0.0.0.0:8787"]

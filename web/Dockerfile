# Packages the built web bundle behind nginx.
#
# The bundle is not compiled here — build it first, then build the image from
# the repository root:
#
#   scripts/build-web.sh
#   docker build -f web/Dockerfile -t ryggattack-web .
#   docker run --rm -p 8080:8080 ryggattack-web
#
# The stable line picks up nginx security updates on rebuild; the one module
# this image depends on is asserted below rather than assumed.

FROM nginx:1.30-alpine

LABEL org.opencontainers.image.title="Ryggattack (web)" \
      org.opencontainers.image.description="The Ryggattack WebAssembly build, served by nginx"

# Fail the build, not the container, if a future base image drops the module
# that serves the pre-compressed bundle.
RUN nginx -V 2>&1 | grep -q -- --with-http_gzip_static_module

COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/web /usr/share/nginx/html

# Roughly 50 MB of WebAssembly gzips to 15 MB. Doing it once at build time
# beats doing it on every download. `gzip -k` is not in busybox, hence `-c`.
RUN gzip -9 -c /usr/share/nginx/html/ryggattack_bg.wasm > /usr/share/nginx/html/ryggattack_bg.wasm.gz \
    && gzip -9 -c /usr/share/nginx/html/ryggattack.js > /usr/share/nginx/html/ryggattack.js.gz \
    && nginx -t

EXPOSE 8080

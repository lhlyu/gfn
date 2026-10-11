# 构建阶段
FROM rust:1.85-alpine AS builder
WORKDIR /build
RUN apk add --no-cache musl-dev
COPY . .
RUN cargo build --release --locked --target x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/gfn

# 运行阶段
FROM alpine:latest
RUN apk add --no-cache ca-certificates git
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/gfn /usr/local/bin/gfn
ENTRYPOINT ["gfn"]

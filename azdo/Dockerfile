FROM rust:1.83-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/azdo /usr/local/bin/azdo
ENV AZDO_PORT=8080 \
    AZDO_DATA=/data/azdo.json
EXPOSE 8080
CMD ["azdo"]

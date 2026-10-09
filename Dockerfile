FROM rust:1-alpine AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/pastebeam /usr/local/bin/

FROM alpine:3

RUN addgroup -S pastebeam && adduser -S -G pastebeam pastebeam

WORKDIR /app
COPY --from=build /usr/local/bin/pastebeam /usr/local/bin/pastebeam
RUN mkdir posts && chown pastebeam:pastebeam posts

USER pastebeam
VOLUME /app/posts
EXPOSE 6969

CMD ["pastebeam"]

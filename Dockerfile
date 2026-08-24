FROM rust:1.89.0-slim-bookworm

RUN rustup component add clippy rustfmt

WORKDIR /workspace
COPY . .

ENV CARGO_TARGET_DIR=/tmp/tetris-target

CMD ["cargo", "test", "--workspace", "--all-targets"]

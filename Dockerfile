FROM rust:bookworm AS build
WORKDIR /app
ADD . .
RUN cargo build -p hookbuffer-standalone --release

FROM debian:bookworm-slim AS runtime
COPY --from=build /app/target/release/hookbuffer-standalone /usr/local/bin/hookbuffer-standalone
CMD ["hookbuffer-standalone"]

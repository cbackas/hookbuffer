FROM rust as build
WORKDIR /app
ADD . .
RUN cargo build -p hookbuffer-standalone --release

FROM rust as runtime
COPY --from=build /app/target/release/hookbuffer-standalone /usr/local/bin/hookbuffer-standalone
CMD ["hookbuffer-standalone"]

FROM rust as build
ADD ./src /app/src
ADD ./Cargo.lock /app/Cargo.lock
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app
RUN cargo build --release

FROM rust as runtime
COPY --from=build /app/target/release/hookbuffer /usr/local/bin/hookbuffer

CMD ["hookbuffer"]


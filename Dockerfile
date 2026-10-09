FROM rust:1.99.0

WORKDIR /app

# we assume the base image is debian
RUN apt-update && app install lld clang -y

COPY . .

RUN cargo build --release

ENTRYPOINT ["./target/release/access-code"]

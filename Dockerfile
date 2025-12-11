FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:stable-slim
WORKDIR /app
COPY --from=builder /app/target/release/zigbee-leak-bot .
COPY .env .env

CMD ["./zigbee-leak-bot"]

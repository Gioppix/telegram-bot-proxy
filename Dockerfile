FROM rust:1.90.0

ARG TELOXIDE_TOKEN
ARG DATABASE_URL

EXPOSE 8080

WORKDIR /app
ENV SQLX_OFFLINE=true
COPY . .
RUN cargo build --release

# Create the data directory for the database
RUN mkdir -p /app/data

CMD ["./target/release/telegram-bot-proxy"]

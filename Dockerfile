# ビルドステージ
FROM rust:1.85.0-slim

# 作業ディレクトリを作成
WORKDIR /app
COPY . .

RUN cargo build --release

CMD [ "./target/release/backend" ]
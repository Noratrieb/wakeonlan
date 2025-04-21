FROM rust as build

WORKDIR /build

COPY . .

RUN cargo build --release

FROM gcr.io/distroless/static-debian12

COPY --from=build /build/target/release/wakeonlan /app/wakeonlan

ENTRYPOINT [ "/app/wakeonlan" ]

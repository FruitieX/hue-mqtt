FROM gcr.io/distroless/cc
COPY target/x86_64-unknown-linux-musl/release/hue-mqtt /usr/local/bin/hue-mqtt
CMD ["hue-mqtt"]

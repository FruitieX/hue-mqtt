FROM gcr.io/distroless/static@sha256:f2ea2709ac8db56323cbd7d014277f32cb572d9ea124b0076f7aafe5980678fe
COPY target/x86_64-unknown-linux-musl/release/hue-mqtt /usr/local/bin/hue-mqtt
CMD ["hue-mqtt"]

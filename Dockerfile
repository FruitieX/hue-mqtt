FROM gcr.io/distroless/static@sha256:a43abc840a7168c833a8b3e4eae0f715f7532111c9227ba17f49586a63a73848
COPY target/x86_64-unknown-linux-musl/release/hue-mqtt /usr/local/bin/hue-mqtt
CMD ["hue-mqtt"]

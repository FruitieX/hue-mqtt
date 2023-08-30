FROM gcr.io/distroless/static@sha256:e7e79fb2947f38ce0fab6061733f7e1959c12b843079042fe13f56ca7b9d178c
COPY target/x86_64-unknown-linux-musl/release/hue-mqtt /usr/local/bin/hue-mqtt
CMD ["hue-mqtt"]

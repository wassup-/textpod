FROM rust:alpine AS build

RUN apk add musl-dev perl make
RUN cargo install textpod
RUN cargo install monolith

FROM alpine
COPY --from=build /usr/local/cargo/bin/textpod /usr/bin/textpod
COPY --from=build /usr/local/cargo/bin/monolith /usr/bin/monolith

RUN apk add --no-cache tzdata curl

RUN apk add --no-cache wget python3 ffmpeg
RUN wget -O /usr/bin/yt-dlp https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp && chmod a+rx /usr/bin/yt-dlp

WORKDIR /app/notes

HEALTHCHECK --interval=60s --retries=3 --timeout=1s \
CMD curl --silent --fail http://localhost:3000/ || exit 1

ENTRYPOINT ["textpod"]
CMD ["-p", "3000", "-l", "0.0.0.0"]

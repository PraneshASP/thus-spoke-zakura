FROM golang:1.25-bookworm AS build
ARG LIGHTWALLETD_COMMIT=d79cd1100575ff909d70e00d5514a4092df94934
RUN git clone https://github.com/zcash/lightwalletd.git /src && cd /src && git checkout "$LIGHTWALLETD_COMMIT"
WORKDIR /src
RUN CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o /lightwalletd ./

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /lightwalletd /usr/local/bin/lightwalletd
ENTRYPOINT ["/usr/local/bin/lightwalletd"]


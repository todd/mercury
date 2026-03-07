# UnrealIRCd for Mercury integration testing.
# Builds UnrealIRCd 6 from source on Debian slim.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libssl-dev \
    libpcre2-dev \
    libargon2-dev \
    libsodium-dev \
    pkg-config \
    wget \
    ca-certificates \
    netcat-openbsd \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Download and build UnrealIRCd 6.
# ./Config -quick requires path env vars set (replaces interactive config.settings).
ARG UNREALIRCD_VERSION=6.1.8

RUN wget -q "https://www.unrealircd.org/downloads/unrealircd-${UNREALIRCD_VERSION}.tar.gz" \
    && tar xzf "unrealircd-${UNREALIRCD_VERSION}.tar.gz" \
    && cd "unrealircd-${UNREALIRCD_VERSION}" \
    # Create the SCRIPTDIR so make install's final `ln` succeeds
    && mkdir -p /opt/unrealircd-script \
    && BINDIR="/opt/unrealircd/bin" \
       DATADIR="/var/lib/unrealircd" \
       CONFDIR="/etc/unrealircd" \
       MODULESDIR="/opt/unrealircd/modules" \
       LOGDIR="/var/log/unrealircd" \
       CACHEDIR="/var/cache/unrealircd" \
       DOCDIR="/opt/unrealircd/doc" \
       TMPDIR="/var/lib/unrealircd/tmp" \
       PRIVATELIBDIR="/opt/unrealircd/lib" \
       SCRIPTDIR="/opt/unrealircd-script" \
       ./Config -nointro -quick \
    && make -j$(nproc) \
    # DESTDIR="" so paths are absolute; ignore non-fatal install errors
    && make install || true \
    && test -f /opt/unrealircd/bin/unrealircd \
    && cd / \
    && rm -rf "unrealircd-${UNREALIRCD_VERSION}" "unrealircd-${UNREALIRCD_VERSION}.tar.gz"

# Create runtime directories and user
RUN useradd -r -s /sbin/nologin -d /var/lib/unrealircd unrealircd \
    && mkdir -p /etc/unrealircd /var/lib/unrealircd /var/lib/unrealircd/tmp \
        /var/log/unrealircd \
        /var/cache/unrealircd /opt/unrealircd/lib /opt/unrealircd/modules \
    && chown -R unrealircd: /etc/unrealircd /var/lib/unrealircd /var/log/unrealircd \
        /var/cache/unrealircd /opt/unrealircd

# Copy our minimal open-server config (overrides the default installed by make install)
COPY unrealircd.conf /etc/unrealircd/unrealircd.conf
RUN touch /etc/unrealircd/ircd.motd /etc/unrealircd/ircd.rules \
    && chown unrealircd: /etc/unrealircd/unrealircd.conf \
        /etc/unrealircd/ircd.motd /etc/unrealircd/ircd.rules

# Generate TLS cert (required by UnrealIRCd even for non-TLS listening)
RUN mkdir -p /etc/unrealircd/tls \
    && openssl req -x509 -newkey rsa:2048 \
        -keyout /etc/unrealircd/tls/server.key.pem \
        -out /etc/unrealircd/tls/server.cert.pem \
        -days 3650 -nodes \
        -subj "/CN=mercury-test.local" \
    && chown -R unrealircd: /etc/unrealircd/tls

EXPOSE 6667

USER unrealircd

HEALTHCHECK --interval=5s --timeout=3s --start-period=30s --retries=15 \
    CMD nc -z 127.0.0.1 6667 || exit 1

CMD ["/opt/unrealircd/bin/unrealircd", "-F"]

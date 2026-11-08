# Backup job image (MB-0022 P5; RR-0014 Q4): sqlite3 for `VACUUM INTO` and
# `integrity_check`, rclone for the upload to Spaces, supercronic as the
# scheduler, tini as PID 1. No compiling: Compose builds it on the host in
# seconds (`docker compose build backup`).
#
# The base image is pinned by tag and digest; packages come from that Alpine
# release branch, pinned to their minor versions (`~`), so a rebuild picks up
# security patch releases but not new features.

FROM alpine:3.24.4@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf

RUN apk add --no-cache \
        'sqlite~3.53' \
        'rclone~1.75' \
        'supercronic~0.2' \
        'tini~0.19' \
        'tzdata'

COPY kauppa-backup /usr/local/bin/kauppa-backup
COPY crontab /etc/kauppa-backup/crontab

# Same uid as the app (deploy/Dockerfile), so a SQLite reader that creates
# kauppa.db-wal or -shm leaves them owned by the app's user.
USER 10001:10001
ENV HOME=/tmp TZ=UTC

ENTRYPOINT ["/sbin/tini", "--", "/usr/local/bin/kauppa-backup"]
CMD ["schedule"]

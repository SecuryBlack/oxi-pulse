# Built by the release workflow from the published Linux binaries, placed at
# dist/amd64/oxipulse and dist/arm64/oxipulse. Run it with the host network and
# the host's / mounted read-only, see "Docker" in the README.
FROM gcr.io/distroless/cc-debian12:nonroot
ARG TARGETARCH
COPY dist/${TARGETARCH}/oxipulse /usr/local/bin/oxipulse
# The image is updated by pulling a new tag, not by the agent itself.
ENV OXIPULSE_AUTO_UPDATE=false
ENTRYPOINT ["/usr/local/bin/oxipulse"]

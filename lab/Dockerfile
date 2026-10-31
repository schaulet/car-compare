FROM python:3.12-slim-bookworm

COPY --from=ghcr.io/astral-sh/uv:0.13.6 /uv /uvx /bin/
COPY --from=docker:27-cli /usr/local/bin/docker /usr/local/bin/docker

WORKDIR /app
COPY pyproject.toml README.md ./
COPY src ./src
RUN uv sync --no-dev --no-editable

ENV PATH="/app/.venv/bin:${PATH}" \
    AZURZ_HOST=0.0.0.0 \
    AZURZ_PORT=8080 \
    AZURZ_DATA=/data \
    AZURZ_IGNITION_ROOT=/ignition

EXPOSE 8080
CMD ["azurz"]

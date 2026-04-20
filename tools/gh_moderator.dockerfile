FROM python:3.12-slim

WORKDIR /app

# Install dependencies
RUN pip install openai pygithub fastapi uvicorn hypy_utils hatchling

# Copy source code
COPY . .

# Set environment variables
ENV PYTHONUNBUFFERED=true
ENV PYTHONPATH=/app

# The app looks for config at ~/.config/gh_moderator.toml
# We will mount it via compose

CMD ["uvicorn", "tools.gh_moderator:app", "--host", "0.0.0.0", "--port", "8000"]

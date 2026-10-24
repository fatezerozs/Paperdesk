FROM python:3.11-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends libgomp1 libgl1 libglib2.0-0 && rm -rf /var/lib/apt/lists/*
WORKDIR /service
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
COPY server.py ./
ENV PADDLE_PDX_CACHE_HOME=/models/paddlex HF_HOME=/models/huggingface MODELSCOPE_CACHE=/models/modelscope
VOLUME ["/models"]
CMD ["python", "-u", "server.py"]

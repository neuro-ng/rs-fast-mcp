#!/bin/bash
set -e

# Container name
CONTAINER_NAME="smart-home-test-diyhue"

# Cleanup function
cleanup() {
  echo "Cleaning up..."
  if [ "$(docker ps -q -f name=$CONTAINER_NAME)" ]; then
    docker stop $CONTAINER_NAME >/dev/null
  fi
  if [ "$(docker ps -aq -f name=$CONTAINER_NAME)" ]; then
    docker rm $CONTAINER_NAME >/dev/null
  fi
}

# Trap exit signals
trap cleanup EXIT

# Clean up any existing container
cleanup

echo "Starting diyhue container..."
# Using create + cp + start to avoid volume mount issues in CI/DooD environments
# where the build path inside the container doesn't match the host path.
docker create \
  --name $CONTAINER_NAME \
  -p 8080:80 \
  -e MAC=00:11:22:33:44:55 \
  diyhue/core:latest

# Copy config file (docker cp works from client to container, bypassing host path issues)
docker cp $(pwd)/examples/smart_home/test_config.json $CONTAINER_NAME:/opt/hue-emulator/config.json

# Start the container
docker start $CONTAINER_NAME

echo "Waiting for diyhue to be ready..."
# Get container IP address (works in Docker-in-Docker setups)
CONTAINER_IP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $CONTAINER_NAME)
echo "Container IP: $CONTAINER_IP"

# Try both localhost (for normal Docker) and container IP (for DinD)
# In CI/DinD environments, localhost port mapping doesn't work, so we use container IP
if [ -n "$CONTAINER_IP" ]; then
  HEALTH_URL="http://$CONTAINER_IP:80/api/config"
else
  HEALTH_URL="http://127.0.0.1:8080/api/config"
fi

echo "Health check URL: $HEALTH_URL"

# Simple health check loop
max_retries=60
count=0
while ! curl -s -m 2 "$HEALTH_URL" > /dev/null; do
  sleep 1
  count=$((count+1))
  if [ $count -ge $max_retries ]; then
    echo "Timeout waiting for diyhue to start"
    echo "Container logs:"
    docker logs $CONTAINER_NAME
    exit 1
  fi
  echo -n "."
done
echo " Ready!"

echo "Running tests..."
# We run the rust integration test here.
# Since it's an example, we might not have a dedicated test target for it unless we define it.
# We will assume a 'cargo test' command specific to this integration test is passed,
# or we just run the example if that's what was intended. 
# But the user asked to translate the 'end to end test'.
# We can compile and run a dedicated test binary or use cargo test.
# Let's run the specific test file we are creating.

cargo test --test integration_test --manifest-path Cargo.toml -- --ignored

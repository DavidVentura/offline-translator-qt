#!/bin/bash
set -ex
./clickable/package-click.sh -a arm64 --skip-review
./venv/bin/clickable install -c clickable.yaml -a arm64  --serial-number 93BAY08W6S  
sleep 0.5
./venv/bin/clickable launch -c clickable.yaml -a arm64  --serial-number 93BAY08W6S  
sleep 2
./venv/bin/clickable logs -c clickable.yaml -a arm64  --serial-number 93BAY08W6S  

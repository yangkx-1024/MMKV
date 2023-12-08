#!/bin/bash

echo "Building Android binary......"
./build_android.sh

echo "Building iOS & Mac binary......"
./build_apple.sh
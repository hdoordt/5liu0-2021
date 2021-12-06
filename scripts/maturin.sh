#!/usr/bin/env bash

# Utility for building and installing Folley as python module
source .pyenv/bin/activate
cd prototype/cli && maturin develop
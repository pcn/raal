#!/bin/bash

cargo graph --optional-line-style dashed --optional-line-color red --optional-shape box --build-shape diamond --build-color green --build-line-color orange  | \
    dot -Tpng > cargo-graph.png 

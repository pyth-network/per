#!/bin/bash

function print_args {
  # express relay program
  echo "--bpf-program PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou ./target/deploy/express_relay.so"
  # dummy program
  echo "--bpf-program DUmmYXYFZugRn2DS4REc5F9UbQNoxYsHP1VMZ6j5U7kZ ./target/deploy/dummy.so"
  # clone limo program from mainnet-beta
  echo "--clone-upgradeable-program LiMoM9rMhrdYrfzUCxQppvxCSG1FcrUK9G8uLq4A1GF -u mainnet-beta"
  # Make sure subscribe block is enabled
  echo "--rpc-pubsub-enable-block-subscription"
  # options
  echo "--reset"
}

print_args

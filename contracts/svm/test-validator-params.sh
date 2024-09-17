#!/bin/bash

function print_args {
  # express relay program
  echo "--bpf-program PytERJFhAKuNNuaiXkApLfWzwNwSNDACpigT3LwQfou ./target/deploy/express_relay.so"
  # dummy program
  echo "--bpf-program DuMYMhmEio5twEWgfYFAvJ6pq215LeCc8MMTPPY4DYUR ./target/deploy/dummy.so"

  # Make sure subscribe block is enabled
  echo "--rpc-pubsub-enable-block-subscription"
  # options
  echo "--reset"
}

print_args

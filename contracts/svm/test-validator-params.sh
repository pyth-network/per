#!/bin/bash

function print_args {
  # express relay program
  echo "--bpf-program GwEtasTAxdS9neVE4GPUpcwR7DB7AizntQSPcG36ubZM ./target/deploy/express_relay.so"
  # dummy program
  echo "--bpf-program HYCgALnu6CM2gkQVopa1HGaNf8Vzbs9bomWRiKP267P3 ./target/deploy/dummy.so"

  # options
  echo "--reset"
}

print_args

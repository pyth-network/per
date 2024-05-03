## Overview

This script is designed to monitor the behavior of blocks and RPC latency for new blocks. Our objective was to ensure that the number of blocks increments sequentially and is always a non-null value. Additionally, we aim to determine whether each new block is mined within a specific time frame or if there is tolerance for variations in mining times.

## Installation

### poetry

```
$ poetry install
```

## Quickstart

To run the block watcher script, run

```
$ poetry -C block-watch run python3 -m block_watch.main > result.csv
```

This simple example runs block watch for specified blocks in the code and write the output to `result.csv` file.

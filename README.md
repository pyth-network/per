# PER

## Off-chain server

Run `uvicorn main:app --reload` to run the FastAPI server. This enables the endpoint for submitting and reading searcher submissions.

## Off-chain auction

In order to run the off-chain auction mechanism, run `python3 -m auction_offchain`. You can run `searcher.py` to submit some prescripted bids to the offchain auction pool, and running `auction_offchain.py` will run the determination for the auction, culminating in a bundle of calls to submit to the multicall contract.

This bundle is automatically saved in an environment file at `.env`. In order to save these variables into your environment, you should run the following commands to source the newly created `.env` file:

```shell
$ set -a
$ source .env
$ set +a
```

The updated enviornment variables can then be seen via `env`. We can then run the appropriate forge tests which will pull the relevant bundle information from the environment variables. To do this, run `forge test -vvv --via-ir --match-test {TestToBeRun}`. Note that you need to `source` the `.env` file in the same session as the one in which you run the forge tests.

### pre-commit hooks

pre-commit is a tool that checks and fixes simple issues (formatting, ...) before each commit. You can install it by following [their website](https://pre-commit.com/). In order to enable checks for this repo run `pre-commit install` from command-line in the root of this repo.

The checks are also performed in the CI to ensure the code follows consistent formatting.

### Development with Tilt

Run `tilt up --namespace dev-<YOUR_NAMESPACE>` to start tilt.

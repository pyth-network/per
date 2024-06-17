// Copyright (C) 2024 Lavra Holdings Limited - All Rights Reserved
pragma solidity ^0.8.13;

import {DummyExchange} from "../../src/DummyExchange.sol";
import {TokenAmount, DummyExchangeExecutionWitness} from "../../src/Structs.sol";
import "permit2/interfaces/ISignatureTransfer.sol";

contract DummyExchangeHarness is DummyExchange {
    constructor(address permit2) {
        _initialize(permit2);
    }

    function exposed_transferSellTokens(
        ISignatureTransfer.PermitBatchTransferFrom calldata permit,
        DummyExchangeExecutionWitness calldata witness,
        bytes calldata signature
    ) public {
        _transferSellTokens(permit, witness, signature);
    }

    function exposed_checkDuplicateTokensTokenAmount(
        TokenAmount[] calldata tokens
    ) public {
        _checkDuplicateTokens(tokens);
    }

    function exposed_checkDuplicateTokensTokenPermissions(
        ISignatureTransfer.TokenPermissions[] calldata tokens
    ) public {
        _checkDuplicateTokens(tokens);
    }

    function exposed_transferBuyTokens(
        DummyExchangeExecutionWitness calldata witness
    ) public {
        _transferBuyTokens(witness);
    }
}

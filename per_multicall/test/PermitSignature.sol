pragma solidity ^0.8.17;

import {Test} from "forge-std/Test.sol";
import {EIP712} from "openzeppelin-contracts/contracts/utils/cryptography/EIP712.sol";
import {ECDSA} from "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import {PermitBatchTransferFrom} from "../src/Structs.sol";
import {Permit2Upgradable} from "../src/Permit2Upgradable.sol";
import "openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

interface EIP712Domain {
    function domainSeparator() external view returns (bytes32);
}

contract PermitSignature is Test {
    bytes32 constant FULL_WITNESS_BATCH_TYPEHASH =
        keccak256(
            "PermitBatchWitnessTransferFrom(TokenPermissions[] permitted,address spender,uint256 nonce,uint256 deadline,OpportunityWitness witness)OpportunityWitness(TokenAmount[] buyTokens,address executor,address targetContract,bytes targetCalldata,uint256 targetCallValue,uint256 bidAmount)TokenAmount(address token,uint256 amount)TokenPermissions(address token,uint256 amount)"
        );

    bytes32 public constant _TOKEN_PERMISSIONS_TYPEHASH =
        keccak256("TokenPermissions(address token,uint256 amount)");

    Permit2Upgradable permit2;

    function setUpPermit2(address admin) public {
        Permit2Upgradable _permit2 = new Permit2Upgradable();
        // deploy proxy contract and point it to implementation
        ERC1967Proxy proxyPermit2 = new ERC1967Proxy(address(_permit2), "");
        permit2 = Permit2Upgradable(payable(proxyPermit2));
        permit2.initialize(admin, admin, admin);
    }

    function getPermitBatchWitnessSignature(
        PermitBatchTransferFrom memory permit,
        uint256 privateKey,
        bytes32 typeHash,
        bytes32 witness,
        address adapter,
        bytes32 domainSeparator
    ) internal returns (bytes memory sig) {
        bytes32[] memory tokenPermissions = new bytes32[](
            permit.permitted.length
        );
        for (uint256 i = 0; i < permit.permitted.length; ++i) {
            tokenPermissions[i] = keccak256(
                abi.encode(_TOKEN_PERMISSIONS_TYPEHASH, permit.permitted[i])
            );
        }

        bytes32 msgHash = keccak256(
            abi.encodePacked(
                "\x19\x01",
                domainSeparator,
                keccak256(
                    abi.encode(
                        typeHash,
                        keccak256(abi.encodePacked(tokenPermissions)),
                        adapter,
                        permit.nonce,
                        permit.deadline,
                        witness
                    )
                )
            )
        );

        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, msgHash);
        return bytes.concat(r, s, bytes1(v));
    }
}

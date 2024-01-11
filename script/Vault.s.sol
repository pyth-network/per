// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console2} from "forge-std/Script.sol";
import "../src/SigVerify.sol";
import "forge-std/StdJson.sol";
import "forge-std/console.sol";
import "forge-std/StdMath.sol";

import {TokenVault} from "../src/TokenVault.sol";
import {SearcherVault} from "../src/SearcherVault.sol";
import {PERMulticall} from "../src/PERMulticall.sol";
import {LiquidationAdapter} from "../src/LiquidationAdapter.sol";
import {MyToken} from "../src/MyToken.sol";
import "../src/Structs.sol";
import "@pythnetwork/pyth-sdk-solidity/MockPyth.sol";

import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

import {WETH9} from "../src/WETH9.sol";

import "openzeppelin-contracts/contracts/utils/Strings.sol";

import "../src/Errors.sol";

contract VaultScript is Script {
    string public latestEnvironmentPath = "latestEnvironment.json";

    function getAnvil() public view returns (address, uint256) {
        // TODO: these are mnemonic wallets. figure out how to transfer ETH from them outside of explicitly hardcoding them here.
        return (
            address(0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266),
            0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
        );
    }

    function deployWeth() public returns (address) {
        (, uint256 skanvil) = getAnvil();
        vm.startBroadcast(skanvil);
        WETH9 weth = new WETH9();
        console.log("deployed weth contract at", address(weth));
        vm.stopBroadcast();
        return address(weth);
    }

    function deployPER(address wethAddress) public returns (address, address) {
        (address perOperatorAddress, uint256 perOperatorSk) = makeAddrAndKey(
            "perOperator"
        );
        console.log("pk per operator", perOperatorAddress);
        console.log("sk per operator", perOperatorSk);
        (, uint256 skanvil) = getAnvil();

        vm.startBroadcast(skanvil);
        payable(perOperatorAddress).transfer(10 ether);
        PERMulticall multicall = new PERMulticall(perOperatorAddress, 0);
        console.log("deployed PER contract at", address(multicall));
        LiquidationAdapter liquidationAdapter = new LiquidationAdapter(
            address(multicall),
            wethAddress
        );
        vm.stopBroadcast();
        return (address(multicall), address(liquidationAdapter));
    }

    function deployVault(
        address multicall,
        address oracle
    ) public returns (address) {
        // make token vault deployer wallet
        (, uint256 tokenVaultDeployerSk) = makeAddrAndKey("tokenVaultDeployer");
        console.log("sk token vault deployer", tokenVaultDeployerSk);
        vm.startBroadcast(tokenVaultDeployerSk);
        TokenVault vault = new TokenVault(multicall, oracle);
        console.log("deployed vault contract at", address(vault));
        vm.stopBroadcast();
        return address(vault);
    }

    function deployMockPyth() public returns (address) {
        (, uint256 skanvil) = getAnvil();
        vm.startBroadcast(skanvil);
        MockPyth mockPyth = new MockPyth(1_000_000, 0);
        console.log("deployed mock pyth contract at", address(mockPyth));
        vm.stopBroadcast();
        return address(mockPyth);
    }

    function deployAll()
        public
        returns (address, address, address, address, address)
    {
        address weth = deployWeth();
        (address per, address liquidationAdapter) = deployPER(weth);
        address mockPyth = deployMockPyth();
        address vault = deployVault(per, mockPyth);
        return (per, liquidationAdapter, mockPyth, vault, weth);
    }

    function setUpContracts() public {
        console.log("balance of this contract", address(this).balance);
        SearcherVault searcherA;
        SearcherVault searcherB;

        MyToken token1;
        MyToken token2;

        bytes32 idToken1;
        bytes32 idToken2;

        address[] memory addressesScript = new address[](5);
        uint256[] memory sksScript = new uint256[](5);

        uint256[] memory qtys;

        // make searcherA and searcherB wallets
        (addressesScript[0], sksScript[0]) = makeAddrAndKey("searcherA");
        (addressesScript[1], sksScript[1]) = makeAddrAndKey("searcherB");
        console.log("sk searcherA", sksScript[0]);
        console.log("sk searcherB", sksScript[1]);

        // make depositor wallet
        (addressesScript[2], sksScript[2]) = makeAddrAndKey("depositor");
        console.log("sk depositor", sksScript[2]);

        // make perOperator wallet
        (addressesScript[3], sksScript[3]) = makeAddrAndKey("perOperator");

        // make tokenVaultDeployer wallet
        (addressesScript[4], sksScript[4]) = makeAddrAndKey(
            "tokenVaultDeployer"
        );

        // TODO: these are mnemonic wallets. figure out how to transfer ETH from them outside of explicitly hardcoding them here.
        (address pkanvil, uint256 skanvil) = getAnvil();

        // transfer ETH to relevant wallets
        vm.startBroadcast(skanvil);
        console.log("balance of pk anvil", pkanvil.balance);
        payable(addressesScript[3]).transfer(10 ether);
        console.log("balance of pk anvil", pkanvil.balance);
        payable(addressesScript[0]).transfer(10 ether);
        console.log("balance of pk anvil", pkanvil.balance);
        payable(addressesScript[1]).transfer(10 ether);
        console.log("balance of pk anvil", pkanvil.balance);
        payable(addressesScript[2]).transfer(10 ether);
        console.log("balance of pk anvil", pkanvil.balance);
        payable(addressesScript[4]).transfer(10 ether);
        vm.stopBroadcast();

        // deploy weth, multicall, liquidationAdapter, oracle, tokenVault
        address multicallAddress;
        address liquidationAdapterAddress;
        address oracleAddress;
        address tokenVaultAddress;
        address wethAddress;
        (
            multicallAddress,
            liquidationAdapterAddress,
            oracleAddress,
            tokenVaultAddress,
            wethAddress
        ) = deployAll();

        // instantiate searcher A's contract with searcher A as sender/origin
        vm.startBroadcast(sksScript[0]);
        console.log("balance of pk searcherA", addressesScript[0].balance);
        searcherA = new SearcherVault(multicallAddress, tokenVaultAddress);
        vm.stopBroadcast();
        console.log("contract of searcher A is", address(searcherA));

        // instantiate searcher B's contract with searcher B as sender/origin
        vm.startBroadcast(sksScript[1]);
        console.log("balance of pk searcherB", addressesScript[1].balance);
        searcherB = new SearcherVault(multicallAddress, tokenVaultAddress);
        vm.stopBroadcast();
        console.log("contract of searcher B is", address(searcherB));

        // fund the searcher contracts
        vm.startBroadcast(skanvil);
        console.log("balance of pkanvil", pkanvil.balance);
        payable(address(searcherA)).transfer(1 ether);
        payable(address(searcherB)).transfer(1 ether);
        vm.stopBroadcast();

        // instantiate ERC-20 tokens
        vm.startBroadcast(sksScript[3]);
        console.log("balance of pk perOperator", addressesScript[3].balance);
        token1 = new MyToken("token1", "T1");
        token2 = new MyToken("token2", "T2");

        console.log("token 1 address", address(token1));
        console.log("token 2 address", address(token2));

        qtys = new uint256[](8); // q_1_dep, q_2_dep, q_1_A, q_2_A, q_1_B, q_2_B, q_1_tokenVault q_2_tokenVault
        qtys[0] = 1_000_000;
        qtys[1] = 1_000_000;
        qtys[2] = 2_000_000;
        qtys[3] = 2_000_000;
        qtys[4] = 3_000_000;
        qtys[5] = 3_000_000;
        qtys[6] = 0;
        qtys[7] = 500_000;

        // mint tokens to the depositor address
        token1.mint(addressesScript[2], qtys[0]);
        token2.mint(addressesScript[2], qtys[1]);

        // mint tokens to searcher A contract
        token1.mint(address(searcherA), qtys[2]);
        token2.mint(address(searcherA), qtys[3]);

        // mint tokens to searcher B contract
        token1.mint(address(searcherB), qtys[4]);
        token2.mint(address(searcherB), qtys[5]);

        // mint token 2 to the vault contract (to allow creation of initial vault with outstanding debt position)
        token2.mint(tokenVaultAddress, qtys[7]);

        // create token price feed IDs--see https://pyth.network/developers/price-feed-ids
        idToken1 = 0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace; // ETH/USD
        idToken2 = 0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43; // BTC/USD
        console.log("ids of tokens");
        console.logBytes32(idToken1);
        console.logBytes32(idToken2);

        // mint token to searchers A and B EOAs
        token1.mint(address(addressesScript[0]), 20_000_000);
        token2.mint(address(addressesScript[0]), 20_000_000);
        token1.mint(address(addressesScript[1]), 30_000_000);
        token2.mint(address(addressesScript[1]), 30_000_000);

        vm.stopBroadcast();

        // searchers A and B approve liquidation adapter to spend their tokens
        vm.startBroadcast(sksScript[0]);
        IERC20(address(token1)).approve(liquidationAdapterAddress, 199_999_999);
        IERC20(address(token2)).approve(liquidationAdapterAddress, 199_999_999);
        // deposit ETH to get WETH
        WETH9(payable(wethAddress)).deposit{value: 1 ether}();
        WETH9(payable(wethAddress)).approve(
            liquidationAdapterAddress,
            399_999_999
        );
        vm.stopBroadcast();
        vm.startBroadcast(sksScript[1]);
        IERC20(address(token1)).approve(liquidationAdapterAddress, 199_999_999);
        IERC20(address(token2)).approve(liquidationAdapterAddress, 199_999_999);
        WETH9(payable(wethAddress)).deposit{value: 1 ether}();
        WETH9(payable(wethAddress)).approve(
            liquidationAdapterAddress,
            399_999_999
        );
        vm.stopBroadcast();

        string memory obj = "latestEnvironment";
        vm.serializeAddress(obj, "tokenVault", tokenVaultAddress);
        vm.serializeAddress(obj, "searcherA", address(searcherA));
        vm.serializeAddress(obj, "searcherB", address(searcherB));
        vm.serializeAddress(obj, "multicall", multicallAddress);
        vm.serializeAddress(
            obj,
            "liquidationAdapter",
            liquidationAdapterAddress
        );
        vm.serializeAddress(obj, "oracle", oracleAddress);

        vm.serializeAddress(obj, "weth", wethAddress);

        vm.serializeAddress(obj, "token1", address(token1));
        vm.serializeAddress(obj, "token2", address(token2));

        vm.serializeBytes32(obj, "idToken1", idToken1);
        vm.serializeBytes32(obj, "idToken2", idToken2);

        vm.serializeAddress(obj, "perOperatorAddress", addressesScript[3]);
        vm.serializeUint(obj, "perOperatorSk", sksScript[3]);
        vm.serializeAddress(obj, "searcherAOwnerAddress", addressesScript[0]);
        vm.serializeUint(obj, "searcherAOwnerSk", sksScript[0]);
        vm.serializeAddress(obj, "searcherBOwnerAddress", addressesScript[1]);
        vm.serializeUint(obj, "searcherBOwnerSk", sksScript[1]);
        vm.serializeAddress(obj, "depositor", addressesScript[2]);
        vm.serializeUint(obj, "depositorSk", sksScript[2]);
        vm.serializeAddress(obj, "tokenVaultDeployer", addressesScript[4]);
        vm.serializeUint(obj, "tokenVaultDeployerSk", sksScript[4]);
        string memory finalJSON = vm.serializeUint(obj, "numVaults", 0);

        vm.writeJson(finalJSON, latestEnvironmentPath);
    }

    function setOraclePrice(
        int64 priceT1,
        int64 priceT2,
        uint64 publishTime
    ) public {
        string memory json = vm.readFile(latestEnvironmentPath);
        address oracleLatest = vm.parseJsonAddress(json, ".oracle");
        bytes32 idToken1Latest = vm.parseJsonBytes32(json, ".idToken1");
        bytes32 idToken2Latest = vm.parseJsonBytes32(json, ".idToken2");

        MockPyth oracle = MockPyth(payable(oracleLatest));

        // set initial oracle prices
        bytes memory token1UpdateData = oracle.createPriceFeedUpdateData(
            idToken1Latest,
            priceT1,
            1,
            0,
            priceT1,
            0,
            publishTime,
            0
        );
        bytes memory token2UpdateData = oracle.createPriceFeedUpdateData(
            idToken2Latest,
            priceT2,
            1,
            0,
            priceT2,
            0,
            publishTime,
            0
        );

        bytes[] memory updateData = new bytes[](2);
        updateData[0] = token1UpdateData;
        updateData[1] = token2UpdateData;
        vm.startBroadcast();
        oracle.updatePriceFeeds(updateData);
        vm.stopBroadcast();

        console.log("token 1, price after:");
        console.logInt(oracle.queryPriceFeed(idToken1Latest).price.price);
        console.log("token 2, price after:");
        console.logInt(oracle.queryPriceFeed(idToken2Latest).price.price);
    }

    function setUpVault(uint256 qT1, uint256 qT2, bool collatT1) public {
        string memory json = vm.readFile(latestEnvironmentPath);
        address depositorLatest = vm.parseJsonAddress(json, ".depositor");
        uint256 depositorSkLatest = vm.parseJsonUint(json, ".depositorSk");
        address tokenVaultLatest = vm.parseJsonAddress(json, ".tokenVault");
        address token1Latest = vm.parseJsonAddress(json, ".token1");
        address token2Latest = vm.parseJsonAddress(json, ".token2");
        bytes32 idToken1Latest = vm.parseJsonBytes32(json, ".idToken1");
        bytes32 idToken2Latest = vm.parseJsonBytes32(json, ".idToken2");
        uint256 numVaults;

        console.log(
            "depositor token balances, before:",
            IERC20(token1Latest).balanceOf(depositorLatest),
            IERC20(token2Latest).balanceOf(depositorLatest)
        );

        if (collatT1) {
            vm.startBroadcast(depositorSkLatest);
            IERC20(token1Latest).approve(address(tokenVaultLatest), qT1);
            numVaults = TokenVault(payable(tokenVaultLatest)).createVault(
                token1Latest,
                token2Latest,
                qT1,
                qT2,
                110 * (10 ** 16),
                1 * (10 ** 16),
                idToken1Latest,
                idToken2Latest
            );
            vm.stopBroadcast();
        } else {
            vm.startBroadcast(depositorSkLatest);
            IERC20(token2Latest).approve(address(tokenVaultLatest), qT2);
            numVaults = TokenVault(payable(tokenVaultLatest)).createVault(
                token2Latest,
                token1Latest,
                qT2,
                qT1,
                110 * (10 ** 16),
                1 * (10 ** 16),
                idToken1Latest,
                idToken2Latest
            );
            vm.stopBroadcast();
        }

        console.log(
            "depositor token balances, after:",
            IERC20(token1Latest).balanceOf(depositorLatest),
            IERC20(token2Latest).balanceOf(depositorLatest)
        );

        vm.writeJson(
            Strings.toString(numVaults),
            latestEnvironmentPath,
            ".numVaults"
        );
    }

    function getBalanceEth(address addy) public view returns (uint256) {
        uint256 balance = addy.balance;
        console.log(balance);
        return balance;
    }

    function getBalanceErc(
        address addy,
        address token
    ) public view returns (uint256) {
        uint256 balance = IERC20(token).balanceOf(addy);
        console.log(balance);
        return balance;
    }

    function getBalanceWeth(address addy) public view returns (uint256) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address wethLatest = vm.parseJsonAddress(json, ".weth");

        uint256 balance = WETH9(payable(wethLatest)).balanceOf(addy);
        console.log(balance);
        return balance;
    }

    function getVault(uint256 vaultID) public view returns (Vault memory) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address tokenVaultLatest = vm.parseJsonAddress(json, ".tokenVault");
        Vault memory vault = TokenVault(payable(tokenVaultLatest)).getVault(
            vaultID
        );
        console.log(
            "vault amounts are",
            vault.amountCollateral,
            vault.amountDebt
        );
        return vault;
    }

    function getAllowances(
        address from,
        address spender
    ) public view returns (uint256, uint256) {
        string memory json = vm.readFile(latestEnvironmentPath);
        address token1Latest = vm.parseJsonAddress(json, ".token1");
        address token2Latest = vm.parseJsonAddress(json, ".token2");
        console.log(
            "allowances are",
            IERC20(token1Latest).allowance(from, spender),
            IERC20(token2Latest).allowance(from, spender)
        );
        console.log(
            "balances are",
            IERC20(token1Latest).balanceOf(from),
            IERC20(token2Latest).balanceOf(from)
        );
        return (
            IERC20(token1Latest).allowance(from, spender),
            IERC20(token2Latest).allowance(from, spender)
        );
    }
}

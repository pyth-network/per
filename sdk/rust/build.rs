use std::{
    env,
    process::Command,
};

fn build_evm_contracts() {
    // Get the directory of the current build.rs file
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let mut contract_path = "../../contracts";
    if current_dir
        .ancestors()
        .any(|ancestor| ancestor.ends_with("target"))
    {
        // If the build.rs file is in the target directory.
        contract_path = "../../../contracts";
    }

    let abis = [
        "OpportunityAdapter",
        "OpportunityAdapterFactory",
        "ERC20",
        "WETH9",
        "ExpressRelay",
    ];

    let contract_setup = format!(
        r#"
        cd {}
        forge build --via-ir
        cd ../sdk/rust
        mkdir -p abi
    "#,
        contract_path
    );

    // Generate `cp` commands for each ABI
    let copy_commands: String = abis
        .iter()
        .map(|abi| format!("cp -r ../../contracts/evm/out/{}.sol ./abi/.", abi))
        .collect::<Vec<_>>()
        .join("\n");

    let full_script = format!("{contract_setup}\n{copy_commands}");
    println!("cargo:rerun-if-changed={}/evm", contract_path);
    // Build the contracts and generate the ABIs. This is required for abigen! macro expansions to work.
    let output = Command::new("sh")
        .args(["-c", full_script.as_str()])
        .output()
        .expect("Failed to run build contracts command");
    if !output.status.success() {
        panic!(
            "Failed to build contracts: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        println!(
            "Built all solidity contracts  {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

fn main() {
    build_evm_contracts();
}

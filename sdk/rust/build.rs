use std::process::Command;


fn build_evm_contracts() {
    let contract_setup = r#"
        cd ../../contracts/evm
        forge build --via-ir
        cp -r ./out ../../sdk/rust/contracts/evm
    "#;
    println!("cargo:rerun-if-changed=../../contracts/evm");
    // Build the contracts and generate the ABIs. This is required for abigen! macro expansions to work.
    let output = Command::new("sh")
        .args(["-c", contract_setup])
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
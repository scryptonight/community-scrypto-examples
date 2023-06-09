use assert_cmd::prelude::*;
use assert_cmd::Command;
use predicates::prelude::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::error::Error;

// These tests will use the `resim` binary to run the tests and
// will change the state of the local resim state.

#[test]
fn publish_package() {
    Setup::new();
}

#[test]
fn test_only_friends_can_deposit() -> Result<(), Box<dyn Error>> {
    let setup = Setup::existing();

    println!("setup: {:?}", setup);

    // instantiate the blueprint
    let mut env_vars = setup.get_env_vars();
    env_vars.insert(
        "account_1".into(),
        setup.default_account.component_address.clone(),
    );
    let friends = vec![TestAccount::resim_new(), TestAccount::resim_new()];
    env_vars.insert("account_2".into(), friends[0].component_address.clone());
    env_vars.insert("account_3".into(), friends[1].component_address.clone());
    println!("env_vars: {:?}", env_vars);

    let cmd = format!("resim run manifests/amount_bound_instantiate.rtm");
    let ouput = run(&cmd, Some(&env_vars));

    // record nft address and component address
    env_vars.insert(
        "component_address".into(),
        parse(&ouput, r"Component: ([a-zA-Z0-9_]+)"),
    );
    let resource_addresses = parse_multiple(&ouput, r"Resource: ([a-zA-Z0-9_]+)");
    let nft_address = resource_addresses.iter().last().unwrap();
    env_vars.insert("nft_address".into(), nft_address.clone());
    println!("env_vars: {:?}", env_vars);

    // deposit from account_1
    env_vars.insert("account".into(), setup.default_account.component_address);
    let cmd = format!("resim run manifests/amount_bound_deposit.rtm",);
    run(&cmd, Some(&env_vars));

    // deposit from account_2
    env_vars.insert("account".into(), friends[0].component_address.clone());
    let cmd = format!(
        "resim run manifests/amount_bound_deposit.rtm -s {}",
        friends[0].private_key
    );
    run(&cmd, Some(&env_vars));

    // deposit from a non-friend account
    let non_friend = TestAccount::resim_new();
    let cmd = format!(
        "resim run manifests/amount_bound_deposit.rtm -s {}",
        non_friend.private_key
    );

    let mut cmd = compose_command(&cmd, Some(&env_vars));

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unauthorized"));

    Ok(())
}

#[test]
fn test_close_early() -> Result<(), Box<dyn Error>> {
    let setup = Setup::existing();

    println!("setup: {:?}", setup);

    // instantiate the blueprint
    let mut env_vars = setup.get_env_vars();
    env_vars.insert(
        "account_1".into(),
        setup.default_account.component_address.clone(),
    );
    let friends = vec![TestAccount::resim_new(), TestAccount::resim_new()];
    env_vars.insert("account_2".into(), friends[0].component_address.clone());
    env_vars.insert("account_3".into(), friends[1].component_address.clone());
    println!("env_vars: {:?}", env_vars);

    let cmd = format!("resim run manifests/amount_bound_instantiate.rtm");
    let ouput = run(&cmd, Some(&env_vars));

    // record nft address and component address
    env_vars.insert(
        "component_address".into(),
        parse(&ouput, r"Component: ([a-zA-Z0-9_]+)"),
    );
    let resource_addresses = parse_multiple(&ouput, r"Resource: ([a-zA-Z0-9_]+)");
    let nft_address = resource_addresses.iter().last().unwrap();
    env_vars.insert("nft_address".into(), nft_address.clone());
    println!("env_vars: {:?}", env_vars);

    let cmd = format!(
        "resim run manifests/amount_bound_close_early.rtm -s {},{},{}",
        setup.default_account.private_key, friends[0].private_key, friends[1].private_key,
    );

    // close early
    run(&cmd, Some(&env_vars));

    Ok(())
}

#[derive(Debug)]
struct Setup {
    default_account: TestAccount,
    package_address: String,
}

impl Setup {
    fn new() -> Self {
        run("resim reset", None);

        let default_account = TestAccount::resim_new();

        let ouput = run("resim publish .", None);
        let package_address = parse(&ouput, r"Success! New Package: ([a-zA-Z0-9_]+)");
        println!("package address: {}", package_address);

        Self {
            default_account,
            package_address,
        }
    }

    fn existing() -> Self {
        let output = run("resim show-configs", None);

        let account_address = parse(&output, r"Account Address: (account_[a-zA-Z0-9_]+)");
        let private_key = parse(&output, r"Account Private Key: ([a-zA-Z0-9_]+)");

        let default_account = TestAccount::new("unknown".into(), private_key, account_address);

        let output = run("resim show-ledger", None);
        let package_address = parse_multiple(&output, r"(package_[a-zA-Z0-9_]+)")
            .iter()
            .last()
            .unwrap()
            .into();

        Self {
            default_account,
            package_address,
        }
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        let mut env_vars = BTreeMap::new();

        env_vars.insert("package_address".into(), self.package_address.clone());

        env_vars.insert(
            "payer_account".into(),
            self.default_account.component_address.clone(),
        );
        env_vars
    }
}

#[derive(Debug)]
struct TestAccount {
    public_key: String,
    private_key: String,
    component_address: String,
}

impl TestAccount {
    fn resim_new() -> Self {
        let resim_output = run("resim new-account", None);
        let public_key = parse(&resim_output, r"Public key: ([a-zA-Z0-9]+)");
        let private_key = parse(&resim_output, r"Private key: ([a-zA-Z0-9]+)");
        let component_address = parse(&resim_output, r"Account component address: ([a-zA-Z0-9_]+)");

        Self {
            public_key,
            private_key,
            component_address,
        }
    }

    fn new(public_key: String, private_key: String, component_address: String) -> Self {
        Self {
            public_key,
            private_key,
            component_address,
        }
    }
}

fn compose_command(cmd: &str, env: Option<&BTreeMap<String, String>>) -> Command {
    let mut input = cmd.split(" ").into_iter();
    let cmd = input.next().unwrap();
    let mut cmd = Command::new(cmd);

    if let Some(env) = env {
        for (key, value) in env {
            cmd.env(key, value);
        }
    }

    for arg in input {
        cmd.arg(arg);
    }

    cmd
}

fn run(cmd: &str, env: Option<&BTreeMap<String, String>>) -> String {
    println!("command: {}", cmd);
    let mut cmd = compose_command(cmd, env);

    let assert = cmd.assert().success();

    let output = &assert.get_output();
    // println!("output: {:?}", output);
    let output = &output.stdout;
    let output = String::from_utf8(output.to_vec()).unwrap();

    println!("output: {}", output);
    output
}

fn parse(output: &str, regex: &str) -> String {
    let re = Regex::new(regex).unwrap();
    let captures = re.captures(output).unwrap();
    captures.get(1).unwrap().as_str().to_string()
}

fn parse_multiple(output: &str, regex: &str) -> Vec<String> {
    let re = Regex::new(regex).unwrap();
    let captures = re.captures_iter(output);
    let mut result = vec![];
    for capture in captures {
        result.push(capture.get(1).unwrap().as_str().to_string());
    }
    result
}

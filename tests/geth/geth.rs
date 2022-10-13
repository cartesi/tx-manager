use std::io::{BufRead, BufReader};
use std::process::Command;
use std::time::{Duration, Instant};

/// How long we will wait for geth to indicate that it is ready.
const GETH_STARTUP_TIMEOUT_MILLIS: u64 = 10_000;

/// The geth command.
const GETH: &str = "geth";

/// The exposed APIs.
const API: &str = "eth,net,web3,txpool,personal,debug";

pub enum Geth {
    Dev { block_time: u16 },
    Goerli,
}

impl Geth {
    fn command(&self) -> &str {
        match self {
            Geth::Dev { block_time: _ } => "geth",
            Geth::Goerli => "geth --goerli",
        }
    }

    fn start(&self, port: u16) -> GethNode {
        let mut cmd = Command::new(self.command());

        // Using stderr for logs.
        cmd.stderr(std::process::Stdio::piped());

        // Opening the HTTP API.
        cmd.arg("--http");
        cmd.arg("--http.port").arg(port.to_string());
        cmd.arg("--http.api").arg(API);

        // Opening the WS API.
        cmd.arg("--ws");
        cmd.arg("--ws.port").arg(port.to_string());
        cmd.arg("--ws.api").arg(API);

        if let Geth::Dev { block_time } = self {
            // Dev mode with custom block times.
            cmd.arg("--dev");
            cmd.arg("--dev.period").arg(block_time.to_string());
        }

        let mut child = cmd.spawn().expect("Could not start geth.");
        let stdout = child
            .stderr
            .expect("Unable to get stderr for geth child process.");

        let start = Instant::now();
        let mut reader = BufReader::new(stdout);

        loop {
            if start + Duration::from_millis(GETH_STARTUP_TIMEOUT_MILLIS) <= Instant::now() {
                panic!("Timed out waiting for geth to start. Is geth installed?")
            }

            let mut line = String::new();
            reader
                .read_line(&mut line)
                .expect("Failed to read line from geth process.");

            // Geth 1.9.23 uses "server started" while 1.9.18 uses "endpoint opened".
            if line.contains("HTTP endpoint opened") || line.contains("HTTP server started") {
                break;
            }
        }

        child.stderr = Some(reader.into_inner());

        GethNode {
            process: child,
            url: format!("http://localhost:{}", port),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------

pub struct GethNode {
    pub url: String,
    process: std::process::Child,
}

impl GethNode {
    pub fn start(port: u16, block_time: u16) -> GethNode {
        let mut cmd = Command::new(GETH);

        // geth uses stderr for its logs
        cmd.stderr(std::process::Stdio::piped());

        // Open the HTTP API
        cmd.arg("--http");
        cmd.arg("--http.port").arg(port.to_string());
        cmd.arg("--http.api").arg(API);

        // Open the WS API
        cmd.arg("--ws");
        cmd.arg("--ws.port").arg(port.to_string());
        cmd.arg("--ws.api").arg(API);

        // Dev mode with custom block time
        cmd.arg("--dev");
        cmd.arg("--dev.period").arg(block_time.to_string());

        let mut child = cmd.spawn().expect("couldnt start geth");

        let stdout = child
            .stderr
            .expect("Unable to get stderr for geth child process");

        let start = Instant::now();
        let mut reader = BufReader::new(stdout);

        loop {
            if start + Duration::from_millis(GETH_STARTUP_TIMEOUT_MILLIS) <= Instant::now() {
                panic!("Timed out waiting for geth to start. Is geth installed?")
            }

            let mut line = String::new();
            reader
                .read_line(&mut line)
                .expect("Failed to read line from geth process");

            // geth 1.9.23 uses "server started" while 1.9.18
            // uses "endpoint opened"
            if line.contains("HTTP endpoint opened") || line.contains("HTTP server started") {
                break;
            }
        }

        child.stderr = Some(reader.into_inner());

        GethNode {
            process: child,
            url: format!("http://localhost:{}", port),
        }
    }

    /*
    pub fn coinbase(&self) -> String {
        let output = Command::new(GETH)
            .args(["attach", &self.url, "--exec", "eth.coinbase"])
            .output()
            .unwrap()
            .stdout;
        let s = std::str::from_utf8(&output).unwrap();
        let hash: String = serde_json::from_str(s).unwrap();
        hash
    }
    */

    pub fn new_account(&self) -> String {
        let output = self.new_command(&"personal.newAccount(\"\")".to_string());
        let s = std::str::from_utf8(&output).unwrap();
        serde_json::from_str(s).unwrap()
    }

    pub fn new_account_with_private_key(&self, private_key: &str) -> String {
        let instruction = format!("personal.importRawKey(\"{}\", \"\")", private_key);
        // println!("instruction: {:?}", instruction);
        let output = self.new_command(&instruction);
        let s = std::str::from_utf8(&output).unwrap();
        serde_json::from_str(s).unwrap()
    }

    pub fn check_balance_in_ethers(&self, hash: &str) -> u64 {
        let mut instruction: String = "web3.fromWei(".to_owned();
        instruction.push_str("eth.getBalance(\"");
        instruction.push_str(hash);
        instruction.push_str("\"), \"ether\")");
        // println!("{:?}", instruction);
        let output = self.new_command(&instruction);
        let s = std::str::from_utf8(&output).unwrap();
        let n: f64 = serde_json::from_str(s).unwrap();
        n as u64
    }

    pub async fn give_funds(&self, to: &str, amount_in_ethers: u64) {
        let balance = self.check_balance_in_ethers(to);
        let mut instruction: String = "personal.sendTransaction(".to_owned();
        instruction.push_str("{from: eth.coinbase, to: \"");
        instruction.push_str(to);
        instruction.push_str("\", value: web3.toWei(");
        instruction.push_str(&amount_in_ethers.to_string());
        instruction.push_str(", \"ether\")}");
        instruction.push_str(", \"\")");
        // println!("{:?}", instruction);
        let output = self.new_command(&instruction);
        let _ = std::str::from_utf8(&output).unwrap();

        // Waiting for the funds to be credited.
        loop {
            if self.check_balance_in_ethers(to) == balance + amount_in_ethers {
                break;
            } else {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    /// Auxiliary.
    fn new_command(&self, instruction: &String) -> Vec<u8> {
        Command::new(GETH)
            .args(["attach", "--exec", instruction, &self.url])
            .output()
            .unwrap()
            .stdout
    }
}

impl Drop for GethNode {
    fn drop(&mut self) {
        self.process.kill().expect("could not kill geth");
    }
}

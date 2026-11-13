//! A standalone fake external tool for integration tests. No shell or cloud access.
use std::{env, fs, io::{self, Read, Write}, path::Path, process, thread, time::Duration};

fn quote(value: &str) -> String {
    format!("\"{}\"",value.replace('\\',"\\\\").replace('"',"\\\"").replace('\n',"\\n").replace('\r',"\\r").replace('\t',"\\t"))
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let tool = Path::new(&args[0]).file_name().unwrap().to_str().unwrap();
    if let Ok(path) = env::var("VTH_TEST_LOG") {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(file,"{{\"program\":{},\"args\":[{}]}}",quote(tool),args[1..].iter().map(|a|quote(a)).collect::<Vec<_>>().join(",")).unwrap();
    }
    let has = |value: &str| args.iter().any(|a|a == value);
    if tool == "aws" {
        if has("get-caller-identity") { println!("{{\"Account\":\"123456789012\"}}"); }
        else if has("head-bucket") && env::var("VTH_TEST_MODE").as_deref() == Ok("forbidden") { eprintln!("An error occurred (403) when calling HeadBucket: Forbidden"); process::exit(1); }
        else if has("start-query-execution") { println!("{{\"QueryExecutionId\":\"query-1\"}}"); }
        else if has("get-query-execution") {
            if env::var("VTH_TEST_MODE").as_deref() == Ok("failed-query") { println!("{{\"QueryExecution\":{{\"Status\":{{\"State\":\"FAILED\",\"StateChangeReason\":\"synthetic failure\"}}}}}}"); }
            else { println!("{{\"QueryExecution\":{{\"Status\":{{\"State\":\"SUCCEEDED\"}}}}}}"); }
        }
        else if has("get-query-results") {
            if has("--next-token") {
                println!("{}", fs::read_to_string(env::var("VTH_TEST_PAGE_TWO").unwrap()).unwrap());
            } else { println!("{}", fs::read_to_string(env::var("VTH_TEST_PAGE_ONE").unwrap()).unwrap()); }
        }
        else if has("list-objects-v2") {
            println!("{{\"Contents\":[{{\"Key\":\"name=example_performance/configuration=default/subject=vector/version=0.55.0/timestamp=9/hostname=one/dstat.csv\"}},{{\"Key\":\"name=example_performance/configuration=default/subject=vector/version=0.55.0/timestamp=10/hostname=one/dstat.csv\"}}]}}");
        }
        else if has("cp") && args.last().is_some_and(|a|a == "-") {
            print!("{}", "header\n".repeat(8));
            for i in 0..25 { let mut row = vec!["0".to_owned();19]; row[0] = i.to_string(); row[17] = "2097152".into(); println!("{}",row.join(",")); }
        }
        else if has("get-instance-profile") { println!("{{\"InstanceProfile\":{{\"Roles\":[{{\"RoleName\":\"vector-packer-build\"}}]}}}}"); }
        else { println!("{{}}"); }
    } else if tool == "terraform" && has("apply") && env::var("VTH_TEST_MODE").as_deref() == Ok("failed-terraform") {
        process::exit(17);
    } else if tool == "curl" { println!("203.0.113.8"); }
    else if tool == "vector" || tool == "socat" {
        if tool == "vector" && has("--version") { println!("vector 0.61.0"); return; }
        let mut bytes = vec![];
        io::stdin().read_to_end(&mut bytes).unwrap();
        if let Ok(path) = env::var("VTH_TEST_BYTES") { fs::write(path,bytes.len().to_string()).unwrap(); }
        if let Ok(delay) = env::var("VTH_TEST_DELAY_MS") { thread::sleep(Duration::from_millis(delay.parse().unwrap())); }
        if env::var("VTH_TEST_MODE").as_deref() == Ok("failed-vector") { process::exit(23); }
    } else if tool == "perf" && has("script") { println!("synthetic perf trace"); }
    else if tool == "inferno-collapse-perf" { let mut input = String::new(); io::stdin().read_to_string(&mut input).unwrap(); assert!(!input.is_empty()); println!("root;work 3"); }
    else if tool == "inferno-flamegraph" { let mut input = String::new(); io::stdin().read_to_string(&mut input).unwrap(); assert!(!input.is_empty()); println!("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"); }
    else if tool == "inferno-diff-folded" { assert!(Path::new(&args[1]).is_file()); assert!(Path::new(&args[2]).is_file()); println!("root;work 2 3"); }
}

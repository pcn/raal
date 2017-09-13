// #[macro_use] extern crate lazy_static;
extern crate docopt;
extern crate raal;
extern crate rand;
extern crate shellexpand;


use std::env;
use std::process::Command;
use std::os::unix::process::CommandExt;
use docopt::Docopt;
use rand::{sample, thread_rng};

use raal::ec2_instances::{AshufInfo, read_without_cache, read_via_cache, instances_matching_regex};

const USAGE: &'static str = "
Query amazon for a random choice among some set of resources

Display matching resources as a JSON document.

Usage:
  ashuf [-c] [-d] [-e <env_name>] [-t <cache_dir>] [-r <region>...] <pattern> [<more_ssh_options>...]
  ashuf (-h | --help)

Options:
  -h --help                 Show this help screen
  -d                        Debug stuff
  -c                        Bypass the cached resources info
  -e --env-name=<env_name>  The environment variable containing the name of this account [default: AWS_ACCOUNT_ID]
  -r --region=<region>      Region (can be specified more than once) [default: us-east-1 us-west-2]
  -s --ssh-command=<cmd>    Path to ssh or a wrapper [default: /usr/bin/ssh]
  -t <cache_dir>            Directory for cached data [default: ~/.raal]

";

fn launch_ssh(ssh_path: String, more_ssh_options: Vec<String>, info: AshufInfo) {
    println!("Mark");
    let mut args = vec!["-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null" ];

    for arg in &more_ssh_options {
        args.push(&arg);
    }
    args.push(&info.private_ip_addresses[0]);
    
    let could_not_exec = Command::new(ssh_path.clone())
        .args(args.clone())
        .exec();

    // let could_not_exec = Command::new("/usr/bin/env").exec();
    println!("Couldn't exec {} {:?} because {:?}", ssh_path, args, could_not_exec);
}


fn main() {
    let version = "0.1.0".to_owned();
    let parsed_cmdline = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(version)).parse())
        .unwrap_or_else(|e| e.exit());
    let debug = parsed_cmdline.get_bool("-d");
    let pattern = parsed_cmdline.get_str("<pattern>").to_string();
    if debug {
        println!("Command line parsed to {:?}", parsed_cmdline);
        println!("Pattern is {:?}", pattern);
    };
    let r = parsed_cmdline.get_vec("-r");
    let aws_id = match env::var(parsed_cmdline.get_str("-e")) {
        Ok(val) => val,
        Err(_) => "default".to_string()
    };

    let bypass_cache = parsed_cmdline.get_bool("-c");
    let cache_ttl = 300;
    let cache_dir = shellexpand::full(parsed_cmdline.get_str("-t"))
        .unwrap()
        .to_string();

    let all_instances = match bypass_cache {
        true => {
            println!("Bypassing the cache");
            read_without_cache(&cache_dir, &r[0].to_string(), &aws_id)
        },
        false => read_via_cache(&cache_dir, &r[0].to_string(), &aws_id, cache_ttl),
    };
    // These are the tags we'll filter on
    let tags = vec!["Name".to_string(), "Tier".to_string()];
    let matches = instances_matching_regex(pattern, tags, all_instances);
    let ssh_path = parsed_cmdline.get_str("-s");
    let more_ssh_options = {
        let ssh_options = parsed_cmdline.get_vec("<more_ssh_options>");
        let mut opts = Vec::new();
        for opt in &ssh_options {
            opts.push(opt.to_string());
        };
        opts
    };
    let mut rng = thread_rng();
    let sampled_instance = sample(&mut rng, matches.clone(), 1);
    if sampled_instance.len() == 0 {
        println!("The list of matches is {:?}", matches);
        println!("But the sample returned is 0 length");
    } else {
        println!("{:?}", sampled_instance[0]);
    }
    launch_ssh(ssh_path.to_string(), more_ssh_options, sampled_instance[0].clone());
}

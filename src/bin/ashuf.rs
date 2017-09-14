// #[macro_use] extern crate lazy_static;
extern crate docopt;
extern crate raal;
extern crate rand;
extern crate shellexpand;

use std::process::Command;
use std::os::unix::process::CommandExt;
use docopt::Docopt;
use rand::{sample, thread_rng};

use raal::ec2_instances::{AshufInfo, read_without_cache, read_via_cache, instances_matching_regex, running_instances};
use raal::config::read_config ;

const USAGE: &'static str = "
Query amazon for a random choice among some set of resources

Display matching resources as a JSON document.

Usage:
  ashuf [-c] [-v] [-d <data_dir>] [-n <name>] <pattern> [<more_ssh_options>...]
  ashuf (-h | --help)

Options:
  -h --help                 Show this help screen
  -v                        verbose info for troubleshooting
  -c                        Bypass the cached resources info
  -s --ssh-command=<cmd>    Path to ssh or a wrapper [default: /usr/bin/ssh]
  -d <data_dir>             Data directory with cached data and config [default: ~/.raal]
  -n <name>                 Easy name for this environment [default: default]

";

fn launch_ssh(ssh_path: String, more_ssh_options: Vec<String>, info: AshufInfo) {
    let mut args = vec!["-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null" ];

    for arg in &more_ssh_options {
        args.push(&arg);
    }
    args.push(&info.private_ip_addresses[0]);
    
    let could_not_exec = Command::new(ssh_path.clone())
        .args(args.clone())
        .exec();
    println!("Couldn't exec {} {:?} because {:?}", ssh_path, args, could_not_exec);
}


fn main() {
    let version = "0.1.0".to_owned();
    let parsed_cmdline = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(version)).parse())
        .unwrap_or_else(|e| e.exit());
    let debug = parsed_cmdline.get_bool("-v");
    let pattern = parsed_cmdline.get_str("<pattern>").to_string();
    if debug {
        println!("Command line parsed to {:?}", parsed_cmdline);
        println!("Pattern is {:?}", pattern);
    };
    let env_name = parsed_cmdline.get_str("-n");
    let bypass_cache = parsed_cmdline.get_bool("-c");
    let cache_ttl = 3600;
    let data_dir = shellexpand::full(parsed_cmdline.get_str("-d"))
        .unwrap()
        .to_string();
    let config = read_config(&data_dir); 
    let aws_id = config.environments
        .get(&env_name.to_string())
        .unwrap()
        .account_id
        .clone();
    let aws_region = config.environments
        .get(&env_name.to_string())
        .unwrap()
        .region
        .clone();
    
    let all_instances = match bypass_cache {
        true => {
            if debug {
                println!("Bypassing the cache");
            }
            read_without_cache(&data_dir, &aws_region, &aws_id)
        },
        false => read_via_cache(&data_dir, &aws_region, &aws_id, cache_ttl),
    };
    // These are the tags we'll filter on
    let tags = vec!["Name".to_string(), "Tier".to_string()];
    let matches = instances_matching_regex(pattern, tags, all_instances);
    let alive_matches = running_instances(matches);
    let ssh_path = parsed_cmdline.get_str("-s");

    // Allow the configured ssh options to be overridden
    let more_ssh_options = {
        let conf_opts = match config.environments.get(&env_name.to_string()) {
            Some(cf) => cf.ssh_options.clone(),
            None => vec![]
        };

        if parsed_cmdline.get_vec("<more_ssh_options>").len() > 0 {
            parsed_cmdline.get_vec("<more_ssh_options>")
                .into_iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
        } else {
            conf_opts
        }
    };

    let mut rng = thread_rng();
    let sampled_instance = sample(&mut rng, alive_matches.clone(), 1);
    if sampled_instance.len() == 0 {
        println!("The list of matches is {:?}", alive_matches);
        println!("And the sample returned is 0 length");
        println!("No instances matched your request, not doing anything");
    } else {
        if debug {
            println!("{:?}", sampled_instance[0]);
        } else {
            println!("Name: {} IP: {} SSH options: {:?}",
                    sampled_instance[0].tags.get("Name").unwrap(),
                    sampled_instance[0].private_ip_addresses[0],
                     more_ssh_options);
        launch_ssh(ssh_path.to_string(), more_ssh_options, sampled_instance[0].clone());
        }
    }
}

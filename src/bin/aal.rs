// #[macro_use] extern crate lazy_static;
extern crate docopt;
extern crate raal;
extern crate regex;
extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate shellexpand;


use std::env;
use docopt::Docopt;

use raal::ec2_instances::{AshufInfo, read_via_cache, instances_matching_regex};

const USAGE: &'static str = "
Query amazon for a random choice among some set of resources

Display matching resources as a JSON document.

Usage:
  aal [-c | --no-cache] [-e <env_name>]  [-d | --debug] [-m <output_mode>] [-a <api>...] [-r <region>...] [-t <cache_dir>] <pattern>
  aal (-h | --help)

Options:
  -h --help                 Show this help screen
  -d --debug                whatever stuff I've broken will get done
  -a --api=<api>            Which AWS api [default: ec2]
  -c --no-cache             Bypass the cached resources info
  -e --env-name=<env_name>  The environment variable containing the name of this account [default: AWS_ACCOUNT_ID]
  -m --mode=<output_mode>   Output mode [default: json_ashuf_info]
  -t <cache_dir>            Directory for cached data [default: ~/.raal]
  -r --region=<region>      Region (can be specified more than once) [default: us-east-1 us-west-2]

Output modes include: ip_private_line, json_ashuf_info, enum_name_tag
";

fn print_ip_private_line(results: Vec<AshufInfo>) {
    // prints the public ip addresses of matches, on per line
    for r in results {
        for addr in r.private_ip_addresses {
            println!("{}", addr);
        };
    };
}

fn print_json_ashuf_info(results: Vec<AshufInfo>) {
    // prints the public ip addresses of matches, as json
    println!("{}", serde_json::to_string_pretty(&results).expect("Couldn't serialize config"));
}

fn print_enum_name_tag(results: Vec<AshufInfo>) {
    // prints a list of the names:addresses of instances, one pre line
    println!("When this works, sort and print a list, with numbers, of matches");
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
    let cache_dir = shellexpand::full(parsed_cmdline.get_str("-t"))
        .unwrap()
        .to_string();
    let aws_id = match env::var(parsed_cmdline.get_str("-e")) {
        Ok(val) => val,
        Err(_) => "default".to_string()
    };

    let cache_ttl = 300;

    let all_instances = read_via_cache(&cache_dir, &r[0].to_string(), &aws_id, cache_ttl);
    // These are the tags we'll filter on
    let tags = vec!["Name".to_string(), "Tier".to_string()];
    let matches = instances_matching_regex(pattern, tags, all_instances);
    // let matched_json = serde_json::to_string_pretty(&matches).expect("Couldn't serialize config");
    let output_format = parsed_cmdline.get_str("-m");

    if output_format == "ip_private_line" {
        print_ip_private_line(matches);
    } else if output_format == "json_ashuf_info" {
        print_json_ashuf_info(matches);
    } else if output_format == "enum_name_tag" {
        print_enum_name_tag(matches);
    }
}

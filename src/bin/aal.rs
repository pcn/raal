// #[macro_use] extern crate lazy_static;
extern crate docopt;
extern crate raal;
extern crate serde_json;
extern crate shellexpand;

use docopt::Docopt;

use raal::ec2_instances::{AshufInfo, read_without_cache, read_via_cache, instances_matching_regex};
use raal::config::read_config;

const USAGE: &'static str = "
Query amazon for a random choice among some set of resources

Display matching resources as a JSON document.

Usage:
  aal [-c | --no-cache] [-e <env_name>] [-d <data_dir>] [-m <output_mode>]  [-n <name>] <pattern>
  aal (-h | --help)

Options:
  -h --help                 Show this help screen
  -d <data_dir>             Data directory with cached data and config [default: ~/.raal]
  -c --no-cache             Bypass the cached resources info
  -e --env-name=<env_name>  The environment variable containing the name of this account [default: AWS_ACCOUNT_ID]
  -m --mode=<output_mode>   Output mode [default: json_ashuf_info]
  -n <name>                 Easy name for this environment [default: default]

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

// fn print_enum_name_tag(results: Vec<AshufInfo>) {
//     // prints a list of the names:addresses of instances, one pre line
//     println!("When this works, sort and print a list, with numbers, of matches");
// }


fn main() {
    let version = "0.1.0".to_owned();
    let parsed_cmdline = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(version)).parse())
        .unwrap_or_else(|e| e.exit());
    let pattern = parsed_cmdline.get_str("<pattern>").to_string();
    let debug = false;
    // if debug {
    //     println!("Command line parsed to {:?}", parsed_cmdline);
    //     println!("Pattern is {:?}", pattern);
    // };

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
    // let matched_json = serde_json::to_string_pretty(&matches).expect("Couldn't serialize config");
    let output_format = parsed_cmdline.get_str("-m");

    if output_format == "ip_private_line" {
        print_ip_private_line(matches);
    } else if output_format == "json_ashuf_info" {
        print_json_ashuf_info(matches);
    // } else if output_format == "enum_name_tag" {
        // print_enum_name_tag(matches); 
    }
}

// #[macro_use] extern crate lazy_static;


extern crate docopt;
extern crate rusoto;
extern crate rush;
extern crate regex;
extern crate rand;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;


use std::collections::{HashMap, HashSet};
use rusoto::Region;
use rusoto::ec2::{Ec2Client, DescribeInstancesRequest, Instance};
use rusoto::default_tls_client;
use std::str::FromStr;
use docopt::Docopt;
use regex::Regex;

use rush::ec2_instances::{AshufInfo, write_saved_json};

const USAGE: &'static str = "
Query amazon for a random choice among some set of resources

Display matching resources as a JSON document.

Usage:
  aal [-c | --no-cache] [-d | --debug] [-a <api>...] [-r <region>...] <pattern>
  aal (-h | --help)

Options:
  -h --help             Show this help screen
  -c --no-cache         Bypass the cached resources info
  -r --region=<region>  Region (can be specified more than once) [default: us-east-1 us-west-2]
  -a --api=<api>        Which AWS api [default: ec2]
  -s --ssh-host         Pick a node to ssh to
  -d --debug            whatever stuff I've broken will get done
";


// // A flat structure to make searching for an instance faster, with a
// // link back to the instance.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// struct AshufInfo {
//     instance_id: String,
//     private_ip_addresses: Vec<String>,
//     public_ip_addresses: Vec<String>,
//     state_name: String,
//     launch_time: String,
//     availability_zone: String,
//     image_ami: String,
//     tags: HashMap<String, String>,
// }

fn ip_addresses_of(instance: &Instance) -> (Vec<String>, Vec<String>) {
    /// A host can have either an ENI in vpc, or a private IP address from an EIP (classic)
    /// This function extracts those addresses, and returns two vectors.  The left
    /// vector contains the private addresses of an instance, and the right vector contains the
    /// public addresses of an instance.
    let mut private = HashSet::new();
    let mut public = HashSet::new();

    if let Some(ref network_interfaces) = instance.network_interfaces {
        for interface in network_interfaces {
            if let Some(ref addr) = interface.private_ip_address {
                private.insert(addr.clone());
            }
        }
    }

    instance.private_ip_address.as_ref().map(|addr| private.insert(addr.clone()));
    instance.public_ip_address.as_ref().map(|addr| public.insert(addr.clone()));

    (vec!(private.into_iter().collect()), vec!(public.into_iter().collect()))
}

fn tags_of(instance: &Instance) -> HashMap<String, String> {
    /// Tags are stored as inconvenient pairs of {"Name": "name", "Value": "Value"}
    /// turn them into simpler key/value map here
    let mut tags = HashMap::new();
    if let Some(ref instance_tags) = instance.tags {
        for tag in instance_tags {
             if let (&Some(ref key), &Some(ref val)) = (&tag.key, &tag.value) {
                 tags.insert(key.clone(), val.clone());
             }
         }
     }
    tags
}



fn ashuf_info_list(instances: Vec<Instance>) -> Vec<AshufInfo> {
    /// Take just the data we want for the AshufInfo struct from the
    /// rusoto::ec2::Instance type, and return a vector of `AshufInfo`
    ///
    /// All data is copied from the instances provided, they are consumed
    /// here.
    let mut limited_instances: Vec<AshufInfo> = Vec::new();
    for inst in instances {
        // println!("This instance is {:?}",  inst);
        let (private_addrs, public_addrs) = ip_addresses_of(&inst);
        let tags = tags_of(&inst);
        // println!("{:?}", addrs);
        let new_asi = AshufInfo {
            instance_id: String::from(inst.instance_id.unwrap()),
            private_ip_addresses: private_addrs,
            public_ip_addresses: public_addrs,
            state_name: String::from(inst.state.unwrap().name.unwrap()),
            launch_time: String::from(inst.launch_time.unwrap()),
            availability_zone: String::from(inst.placement.unwrap().availability_zone.unwrap()),
            image_ami: String::from(inst.image_id.unwrap()),
            tags: tags,
        };
        limited_instances.push(new_asi);
    }
    limited_instances
}


// returns OK on the left, and Not OK on the right.
// Let's define that so that on the left are matched instances,
// and the right  is unmatched instances.
fn partition_matches(rexpr: &Regex, tag: &String, instances: Vec<AshufInfo>) -> (Vec<AshufInfo>,  Vec<AshufInfo>) {
    let (matched, unmatched) = instances
        .into_iter()
        .partition(|inst| {
            if let Some(tval) = inst.tags.get(tag) {
                if rexpr.is_match(tval) {  // Match on the value (assuming it can't be None?)
                    // println!("Matched {:?}", tval);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        });
    (matched, unmatched)
}

// Bring
fn instances_matching_regex(pattern: String, interesting_tags: Vec<String>, instances: Vec<AshufInfo>) -> Vec<AshufInfo> {
    let rexpr = Regex::new(&pattern).unwrap();
    let mut unmatched_instances = Vec::new();
    let mut matched_instances = Vec::new();
    unmatched_instances.extend_from_slice(instances.as_slice());

    for ref tag in interesting_tags {
        let (m, u) = partition_matches(&rexpr, tag, unmatched_instances);
        // Re-bind unmatched instances for the next loop
        unmatched_instances = u;
        matched_instances.extend_from_slice(m.as_slice());
    }

    matched_instances
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
    let creds = rush::auth::credentials_provider(None, None);
    // XXx when ready, map over the regions provided and cache those
    // so they can be combined afterwards.  But for now, let's do one
    // region.
    let r = parsed_cmdline.get_vec("-r");

    let reg = Region::from_str(r[0]).unwrap();
    let client = Ec2Client::new(default_tls_client().unwrap(), creds, reg);
    let mut ec2_request_input = DescribeInstancesRequest::default();
    ec2_request_input.instance_ids = None;
    // ec2_request_input.instance_ids = Some(vec!["something".into()]);

    let mut limited_info = Vec::new();

    match client.describe_instances(&ec2_request_input) {
        Ok(response) => {
            let instances = rush::ec2_instances::ec2_res_to_instances(response.reservations.unwrap());
            // if parsed_cmdline.get_bool("-d") {
            //     println!("{:?}", instances);
            // };
            limited_info.extend(ashuf_info_list(instances));
            // println!("{:?}", limited_info.len());

        },
        Err(error) => {
            println!("Error: {:?}", error);
        }
    }

    let tags = vec!["Name".to_string(), "Tier".to_string()];
    let matches = instances_matching_regex(pattern, tags, limited_info);
    // for m in matches.as_ref() {
    //    println!("{}", m.ip_addresses[0].clone());
    // }
    let matched_json = serde_json::to_string_pretty(&matches).expect("Couldn't serialize config");
    // Now write the cache
    match write_saved_json(1, &matches) {
        Ok(_) => println!("{}", matched_json),
        Err(result) => println!("{:?}", result)
    };
}

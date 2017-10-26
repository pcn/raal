extern crate rusoto_core;
extern crate rusoto_ec2;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate chrono;
extern crate regex;
extern crate toml;

pub mod auth {
    use rusoto_core::{ChainProvider, ProfileProvider};
    // From:
    // https://github.comm/InQuicker/kaws/blob/master/src/aws.rs
    #[warn(dead_code)]
    pub fn credentials_provider(path: Option<&str>, profile: Option<&str>) -> ChainProvider {
        let mut profile_provider = ProfileProvider::new().expect(
            "Failed to create AWS credentials provider."
        );

        if let Some(path) = path {
            profile_provider.set_file_path(path);
        }

        if let Some(profile) = profile {
            profile_provider.set_profile(profile);
        }

        ChainProvider::with_profile_provider(profile_provider)
    }
}

pub mod config {

    use std::fs::File;
    use std::io::Read;
    use toml;
    use std::path::Path;
    use std::collections::HashMap;
    
    #[derive(Debug, Deserialize)]
    pub struct Config {
        pub environments: HashMap<String, EnvironConfig>, 
    }
    #[derive(Debug, Deserialize)]
    pub struct EnvironConfig {
        pub account_id: String,
        pub region: String,
        pub ssh_options: Vec<String>,
    }
        

    fn default_config() -> Config {
        let default_config = r#"
[environments.default]
account_id = "123"
region = "us-east-1"
ssh_options = ["-l", "ubuntu", "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null"]
"#;
        match toml::from_str(&default_config) {
            Ok(config) => config,
            Err(err) => {
                println!("The default config couldn't be deserialzed by toml because {:?}. This is a problem in the source: \n{}",
                         err,
                         default_config);
                panic!();
            }
        }
    }

    pub fn read_config(config_dir: &String) -> Config {
        let pathname = format!("{}/config.toml", config_dir);
        let mut file_bytes = String::new();
        let mut config_file = match File::open(Path::new(&pathname)) {
            Ok(file) =>  file,
            Err(err) => {
                println!("Couldn't read config file {} because {:?}", pathname, err);
                panic!();
            }
        };
        
        config_file.read_to_string(&mut file_bytes).expect("Something went wrong");
        match toml::from_str(&file_bytes) {
            Ok(data) => data,
            Err(err) => {
                println!("Couldn't read toml from {} because {:?}", pathname, err);
                println!("Using the default config");
                default_config()
            }
        }
    }
}

// The general idea for saving and restoring paths will be that first the cache will be consulted
// when looking for a resource.  If the resource is found, bingo.
//
// In the case(s) where the resource can't be found, try the API, and if the API call is successful,
// record the updated data.  If it is not so successful, then avoid clobbering the current data.
pub mod ec2_instances {
    use rusoto_core::{Region, default_tls_client};
    use rusoto_ec2::{Ec2, Ec2Client, DescribeInstancesRequest, Instance, Reservation};
    // use std::collections::HashMap;

    use std::fs::{File, rename};
    use std::path::Path;
    use std::io::prelude::*;
    use std::io;
    use std::io::Read;    
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;
    use regex::Regex;



    use chrono::prelude::*;
    use chrono::Duration;

    use serde_json;

    // In the future, this will be a config and a runtime option
    // Also in the future, bless a tuple of environment variable that will
    // distinguish account data (Account, API, region) seems reasonable.
    // "global" seems like an appropriate choice for global APIs like IAM, route53.

    // A flat structure to make searching for an instance faster, with a
    // link back to the instance.

    // TODO: implement a sort on name tags 
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct AshufInfo {
        pub instance_id: String,
        pub private_ip_addresses: Vec<String>,
        pub public_ip_addresses: Vec<String>,
        pub state_name: String,
        pub launch_time: String,
        pub availability_zone: String,
        pub image_ami: String,
        pub tags: HashMap<String, String>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct CacheData { // How we'll cache our data
        written_time: DateTime<Utc>,
        instance_data: Vec<AshufInfo>,
    }


    pub fn read_without_cache(cache_dir: &String, region_name: &String, aws_account_id: &String) -> Vec<AshufInfo> {
        let creds = ::auth::credentials_provider(None, None);
        let reg = Region::from_str(region_name).unwrap();
        let client = Ec2Client::new(default_tls_client().unwrap(), creds, reg);
        
        let mut ec2_request_input = DescribeInstancesRequest::default();
        ec2_request_input.instance_ids = None;
        match client.describe_instances(&ec2_request_input) {
            Ok(response) => {
                let instances = ec2_res_to_instances(response.reservations.unwrap());
                let instances_data = ashuf_info_list(instances);
                match write_saved_json(&aws_account_id, &cache_dir, &region_name, &instances_data) {
                    Ok(msg) => println!("{}", msg),
                    Err(what_happened) => println!("{}", what_happened),
                };
                instances_data
            },
            Err(error) => {
                println!("Error when invoking describe_instances: {:?}", error);
                Vec::new()
            }
        }
    }
        
    
    pub fn read_via_cache(cache_dir: &String, region_name: &String, aws_account_id: &String, cache_ttl: i64) -> Vec<AshufInfo> {
        
        // let mut limited_info = Vec::new();
        // println!("cache_dir: {}", cache_dir);
        // println!("account: {}", aws_account_id);
        // println!("region_name: {}", region_name);
        
        let limited_info = match ec2_cached_data(&cache_dir, &aws_account_id, &region_name, cache_ttl) {
            Ok(instances) => {
                // println!("I'm using cache data");
                instances
            },
            Err(_) => read_without_cache(cache_dir, region_name, aws_account_id)
        };

        // XXx when ready, map over the regions provided and cache those
        // so they can be combined afterwards.  But for now, let's do one
        // region.
        limited_info
    }


    pub fn ip_addresses_of(instance: &Instance) -> (Vec<String>, Vec<String>) {
        // A host can have either an ENI in vpc, or a private IP address from an EIP (classic)
        // This function extracts those addresses, and returns two vectors.  The left
        // vector contains the private addresses of an instance, and the right vector contains the
        // public addresses of an instance.
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

    pub fn tags_of(instance: &Instance) -> HashMap<String, String> {
        // Tags are stored as inconvenient pairs of {"Name": "name", "Value": "Value"}
        // turn them into simpler key/value map here
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
    

    pub fn ashuf_info_list(instances: Vec<Instance>) -> Vec<AshufInfo> {
        // Take just the data we want for the AshufInfo struct from the
        // rusoto::ec2::Instance type, and return a vector of `AshufInfo`
        //
        // All data is copied from the instances provided, they are consumed
        // here.
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
    pub fn partition_matches(rexpr: &Regex, tag: &String, instances: Vec<AshufInfo>) -> (Vec<AshufInfo>,  Vec<AshufInfo>) {
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
    

    pub fn instances_matching_regex(pattern: String, interesting_tags: Vec<String>, instances: Vec<AshufInfo>) -> Vec<AshufInfo> {
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

    pub fn running_instances(instances: Vec<AshufInfo>) -> Vec<AshufInfo> {
        instances.into_iter()
            .filter(|i| i.state_name == "running".to_string())
            .collect::<Vec<AshufInfo>>()
    }

    pub fn ec2_res_to_instances(reservations: Vec<Reservation>) -> Vec<Instance> {
        // The ec2 `describe-instances` call returns a structure that describe
        // reservations, and the reservations contain instances.
        // Since I pretty much never, ever, ever need to know about reservations,
        // and always care about instances, I am removing the reservations info
        // and creating a vector of instances.


        let mut instances = Vec::new();
        for res in reservations {
            for res_instances in res.instances {
                for inst in res_instances {
                    instances.push(inst);
                }
            }
        }
        instances
    }
    

    // I think I want to change how this works.  I think it'd be better if
    // the default  case is that if an instance is found in the cache, then
    // don't refresh, just use it. Only refresh if an instance isn't found
    // and the cache has aged out. Or if the user requests it.
    pub fn ec2_cached_data(cache_dir: &String, account: &String, region_name: &String, cache_ttl: i64) -> Result<Vec<AshufInfo>, String> {
        // Look at the file at the provided path, and if the age of the
        // file is less than the specified age, get ec2 instance info
        // from it instead of from the api.
        // Otherwise go through the API and return that data
        // println!("cache_dir: {}", cache_dir);
        // println!("account: {}", account);
        // println!("region_name: {}", region_name);
        let data = match read_saved_json(&cache_dir, &account, &region_name) {
            Ok(saved_data) => saved_data,
            Err(error) => return Err(format!("{} while opening {}", error, "cache file"))
        };
        let difference = Utc::now().signed_duration_since(data.written_time); // Note that the order matters here.
        println!("Difference in time between cache record and now is {}", difference);
            
        if difference < Duration::seconds(cache_ttl) {
            // println!("Got data, and the time is valid");
            Ok(data.instance_data)
        } else {
            // println!("Got data, and the time expired");            
            Err("Expired".to_string())
        }
    }

    /// This function is for saving the data from a call to the API. It's for
    /// this side-effect only
    // XXX: add support for (API, region)
    pub fn write_saved_json(account: &String, cache_dir: &String, region_name: &String, data: &Vec<AshufInfo>) -> io::Result<String> {
        // Interesting: in rust you can concat a &str onto a String.
        // Deref coercecions may be an interesting topic?
        let pathname = format!("{}/{}_{}_ec2_instances.json", cache_dir, account, region_name);
        
        let tmp_pathname = pathname.to_owned() + ".tmp";

        println!("starting, writing to {}", tmp_pathname);

        let mut cache_file_new = File::create(Path::new(&tmp_pathname))?;
        let cache_data = CacheData {
            written_time: Utc::now(),
            instance_data: data.to_owned(),
        };
        let json_bytes = match serde_json::to_string(&cache_data) {
            Err(_) => "{}".to_string(),
            Ok(output) => output
        };
        cache_file_new.write_all(json_bytes.as_bytes())?;
        // cache_file_new.write_all(serde_json::to_string(&data).unwrap().as_bytes());

        println!("Wrote the tmp cache file");

        rename(tmp_pathname, pathname)?;

        println!("Re-named the local cache file after getting new data");

        Ok("Cache written out".to_string())
    }

    pub fn read_saved_json(cache_dir: &String, account: &String, region_name: &String) -> io::Result<CacheData> {
        let pathname = format!("{}/{}_{}_ec2_instances.json", cache_dir, account, region_name);
        let mut file_bytes = String::new();
        let mut cache_file = match File::open(Path::new(&pathname)) {
            Ok(file) =>  file,
            Err(err) => {
                // This is failing when there is no file, which is bad, since that's how we all start
                // println!("Couldn't read cache file {} because {:?}", pathname, err);
                return Err(err);
            }
        };
        // let mut cache_file = File::open(Path::new(&pathname))?;

        // println!("starting read_saved_json; path to the instances file is known, and file is opened");
        
        cache_file.read_to_string(&mut file_bytes).expect("Something went wrong");
        let instance_data: CacheData = serde_json::from_str(&file_bytes)?;
        Ok(instance_data)
    }
}

extern crate rusoto;
// use rusoto::Region;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;


pub mod auth {
    use rusoto::{ChainProvider, ProfileProvider};
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


// The general idea for saving and restoring paths will be that first the cache will be consulted
// when looking for a resource.  If the resource is found, bingo.
//
// In the case(s) where the resource can't be found, try the API, and if the API call is successful,
// record the updated data.  If it is not so successful, then avoid clobbering the current data.
//
// Also, add a way to generally indicate whether we do want to clobber the cached data

// Note: do I want to have a flag or a struct to define whether or not I should do some of these things?  Like a
// D_NO_CLOBBER_CACHE_FILE or something?
pub mod ec2_instances {
    use rusoto::ec2::{Instance, Reservation};
    use std::collections::HashMap;

    use std::fs::{File, rename};
    use std::path::Path;
    use std::error::Error;
    use std::io::prelude::*;
    use std::io;

    use serde_json;

    static cache_dir: &'static str= "/tmp/"; // In the future, this will be a config and a runtime option


    // A flat structure to make searching for an instance faster, with a
    // link back to the instance.

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


    pub fn ec2_res_to_instances(reservations: Vec<Reservation>) -> Vec<Instance> {
        /// The ec2 `describe-instances` call returns a structure that describe
        /// reservations, and the reservations contain instances.
        /// Since I pretty much never, ever, ever need to know about reservations,
        /// and always care about instances, I am removing the reservations info
        /// and creating a vector of instances.


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


    // fn get_saved_json(cache_path_dir, api, age) -> Option(<File>) {
    //     /// Look at the file at the provided path, and if the age of the
    //     /// file is less than the specified age, get ec2 instance info
    //     /// from it instead of from the api.

    //     let specified_age = 600; // seconds
    // }

    pub fn write_saved_json(age: i32, data: &Vec<AshufInfo>) -> io::Result<()> {
        // Interesting: in rust you can concat a &str onto a String.
        // Deref coercecions may be an interesting topic?
        let pathname = cache_dir.clone().to_owned() + "ec2_instances.json";
        let tmp_pathname = pathname.to_owned() + "tmp";

        println!("starting");

        let mut cache_file_new = File::create(Path::new(&tmp_pathname))?;
        let json_bytes = match serde_json::to_string(data) {
            Err(_) => "{}".to_string(),
            Ok(output) => output
        };
        cache_file_new.write_all(json_bytes.as_bytes())?;
        // cache_file_new.write_all(serde_json::to_string(&data).unwrap().as_bytes());

        println!("Wrote the tmp cache file");

        rename(tmp_pathname, pathname)?;

        println!("Re-named the local cache file after getting new data");

        Ok(())

    }
}

extern crate rusoto;
use rusoto::Region;
use rusoto::ec2::{Ec2Client, DescribeInstancesRequest, Reservation, Instance};

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


pub mod ec2_instances {
    use rusoto::ec2::{Instance, Reservation};
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
}

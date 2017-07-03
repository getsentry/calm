use std::io;

use clap;
use serde_yaml;
use serde_json;


error_chain! {
    errors {
        QuietExit(code: i32) {
            description("calm quit")
        }
    }

    foreign_links {
        Io(io::Error);
        Clap(clap::Error);
        Yaml(serde_yaml::Error);
        Json(serde_json::Error);
    }
}

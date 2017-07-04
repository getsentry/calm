use std::io;

use clap;
use serde_yaml;
use serde_json;
use git2;
use elementtree;


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
        Git(git2::Error);
        Xml(elementtree::Error);
    }
}

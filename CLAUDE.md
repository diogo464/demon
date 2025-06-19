## project development guidelines
remember to use a ./IMPLEMENTATION_PLAN.md file to keep track of your work and maintain it updated when you complete work or requirements changes. you should add as much detail as you think is necessary to this file.

## rust guidelines
do not add dependencies manually, instead, use the following tools:
+ `cargo info` to obtain information about a crate such as its version, features, licence, ...
+ `cargo add` to add new dependencies, you can use the `--features` to specifiy comma separated list of features
+ for logging, prefer the `tracing` crate with `tracing-subscriber` and fully qualify the log macros (ex: `tracing::info!`)
+ for cli use the `clap` crate. when implementing subcommands use an `enum` and separate structs for each subcommand's arguments
+ use the `anyhow` crate for error handling

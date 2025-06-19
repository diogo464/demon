# project goals
the goal of this project is to create a cli tool named `demon` that should be used to spawn background processes and redirect their stdout and stderr to files.
here is an overview of the subcommands the tool should provide and their usage:

## demon run
```
# the identifier is a required argument
# all remaining arguments will be used to spawn the process with the first one being the executable name
# three files should be created `.pid`, `.stdout`, `.stderr`
# the cli tool should exit immediatly but the spawned process should be left running the background
demon run --id <identifier> <command...>

# example usage
demon run --id npm-dev npm run dev
# this should create the files `npm-dev.pid`, `npm-dev.stdout` and `npm-dev.stderr`
# if the pid file already exists and the process is still running you should fail with a descriptive error message
```

## demon stop
```
# this should kill the process if it is running, otherwise do nothing
demon stop --id <id>
```

## demon tail
```
# this should tail both .stderr and .stdout files by default, or just the selected ones
demon tail [--stdout] [--stderr] --id <id>
```

## demon cat
```
# this should cat both files or just the selected ones
demon cat [--stdout] [--stderr] --id <id>
```

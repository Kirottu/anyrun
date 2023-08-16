# Nix-Run

Launch nix applications without installing them.

## Usage

Simply search for the application you wish to launch.

## Configuration

```ron
// <Anyrun config dir>/nix-run.ron
Config(
  // The prefix that the search needs to begin with to yield results
  prefix: "nixpkgs#"
  max_entries: 5,
)
```
